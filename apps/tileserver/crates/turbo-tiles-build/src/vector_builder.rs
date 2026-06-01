//! Generic vector-feature builder. Reads geometry rows from
//! Postgres and writes them as one or more named collections into
//! `norway.vectors`.
//!
//! The configuration is a TOML file (defaulting to
//! `tools/vector-layers.toml`) so adding a new feature class —
//! avalanche zones, fences, restricted areas, point hazards — is a
//! one-row change. No new code, no schema migration.
//!
//! ## Config shape
//!
//! ```toml
//! [[layer]]
//! name        = "water"
//! table       = "n50_staging.vann_omrade"
//! geom_column = "omrade"
//! kind        = "polygon"
//! attrs       = [
//!   { name = "area_m2", ty = "f32", source = "ST_Area(omrade)::float4" },
//! ]
//!
//! [[layer]]
//! name        = "streams"
//! table       = "n50_staging.elvbekk"
//! geom_column = "senterlinje"
//! kind        = "linestring"
//! attrs       = [
//!   { name = "width_m", ty = "f32", source = "COALESCE(bredde::float4, 2.0)" },
//! ]
//! ```
//!
//! Polygons with multiple rings (holes) are exploded to one feature
//! per outer ring — the consuming layers don't model holes, and N50
//! water/wetland polygons we've seen don't contain any. If that
//! changes the format manifest can be extended without breaking
//! readers (manifest is JSON, attr_schema is per-collection).

use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::info;
use turbo_tiles_artifacts::ArtifactKind;
use turbo_tiles_db::DbPool;
use turbo_tiles_geom::Point;
use turbo_tiles_vector::{
    write_store_to_path, AttrField, AttrSchema, AttrType, CollectionBuilder, GeomKind,
};

use crate::BuildError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorLayerSpec {
    pub name: String,
    /// Fully qualified table or view, e.g. `n50_staging.vann_omrade`.
    pub table: String,
    pub geom_column: String,
    pub kind: GeomKind,
    /// Optional SQL WHERE clause (without the `WHERE` keyword).
    #[serde(default)]
    pub r#where: Option<String>,
    /// Attribute fields. Each pulls a value from a SQL expression in
    /// the SELECT (e.g. `bredde::float4`, `ST_Area(omrade)::float4`)
    /// and writes its bytes at a fixed offset in the per-feature blob.
    #[serde(default)]
    pub attrs: Vec<VectorAttrSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorAttrSpec {
    pub name: String,
    pub ty: AttrType,
    /// SQL expression to evaluate. Must be safe to embed in a SELECT;
    /// upstream config is curator-controlled, not user-facing.
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorConfig {
    #[serde(rename = "layer")]
    pub layers: Vec<VectorLayerSpec>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VectorBuildReport {
    pub out_path: PathBuf,
    pub collections: Vec<VectorCollectionReport>,
    pub file_size_bytes: u64,
    pub seconds: f64,
    /// Aggregated health audit over all vector collections. Flags
    /// the empty-collection / sparse-geometry patterns that show up
    /// when an upsert silently drops rows (the N50 hash-collision
    /// pattern that bit this session).
    pub health: crate::health::HealthReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct VectorCollectionReport {
    pub name: String,
    pub kind: GeomKind,
    pub feature_count: u32,
    pub total_vertices: u32,
}

pub async fn build_from_config(
    pool: &DbPool,
    out_dir: &Path,
    config: &VectorConfig,
) -> Result<VectorBuildReport, BuildError> {
    let started = Instant::now();
    std::fs::create_dir_all(out_dir)?;
    let mut collections: Vec<CollectionBuilder> = Vec::new();
    let mut reports: Vec<VectorCollectionReport> = Vec::new();
    for spec in &config.layers {
        info!(layer = spec.name.as_str(), "building vector layer");
        let (cb, report) = build_one_layer(pool, spec).await?;
        collections.push(cb);
        reports.push(report);
    }
    let out_path = out_dir.join(ArtifactKind::Vectors.filename());
    let file_size_bytes = write_store_to_path(&out_path, collections, Utc::now().timestamp())
        .map_err(|e| BuildError::Logic(format!("vector store write: {e}")))?;
    // Per-collection health audit. Empty layer → likely ingest
    // bug. Sparse vertex count → likely WKB parse fallback. Both
    // ride along on the report.
    let mut health = crate::health::HealthReport::default();
    for r in &reports {
        health.stat(
            &format!("vector_{}_features", r.name),
            r.feature_count as f64,
        );
        health.stat(
            &format!("vector_{}_vertices", r.name),
            r.total_vertices as f64,
        );
        for issue in crate::health::audit_vector_layer(&r.name, r.feature_count, r.total_vertices) {
            tracing::warn!(code = %issue.code, "{}", issue.message);
            health.warnings.push(issue);
        }
    }
    let health_path = out_dir.join("norway.vectors.health.json");
    let body = serde_json::to_vec_pretty(&serde_json::json!({
        "written_at_unix_sec": Utc::now().timestamp(),
        "report": &health,
    }))
    .unwrap_or_default();
    let _ = std::fs::write(&health_path, &body);
    Ok(VectorBuildReport {
        out_path,
        collections: reports,
        file_size_bytes,
        seconds: started.elapsed().as_secs_f64(),
        health,
    })
}

async fn build_one_layer(
    pool: &DbPool,
    spec: &VectorLayerSpec,
) -> Result<(CollectionBuilder, VectorCollectionReport), BuildError> {
    // Compute attr layout: each field gets a fixed offset into the
    // per-feature blob. Fields are laid out in declaration order.
    let mut fields: Vec<AttrField> = Vec::with_capacity(spec.attrs.len());
    let mut offset = 0u32;
    for a in &spec.attrs {
        fields.push(AttrField {
            name: a.name.clone(),
            ty: a.ty,
            offset,
        });
        offset += a.ty.size();
    }
    let bytes_per_feature = offset;
    let schema = AttrSchema {
        fields,
        bytes_per_feature,
    };

    // SQL projection: ST_AsBinary(geom) plus each attribute source
    // expression. Geometry comes back as little-endian WKB. We use
    // ST_Force2D so any 3D leftovers from elevation joins decay
    // cleanly (the integral cost layers are 2D anyway).
    let mut select_cols: Vec<String> = vec![format!(
        "ST_AsBinary(ST_Force2D({})) AS geom_wkb",
        spec.geom_column
    )];
    for a in &spec.attrs {
        select_cols.push(format!("({}) AS \"{}\"", a.source, a.name));
    }
    let where_clause = match spec.r#where.as_ref() {
        Some(w) => format!("WHERE {w}"),
        None => String::new(),
    };
    let sql = format!(
        "SELECT {} FROM {} {} ",
        select_cols.join(", "),
        spec.table,
        where_clause,
    );

    let mut rows = sqlx::query(&sql).fetch(pool);
    let mut cb = CollectionBuilder::new(spec.name.clone(), spec.kind, schema.clone());
    let mut total_vertices: u32 = 0;
    while let Some(row) = rows.try_next().await? {
        let wkb: Vec<u8> = row.try_get::<Vec<u8>, _>("geom_wkb").unwrap_or_default();
        if wkb.is_empty() {
            continue;
        }
        let mut attr_buf = vec![0u8; bytes_per_feature as usize];
        encode_attrs(&spec.attrs, &row, &mut attr_buf)?;
        match spec.kind {
            GeomKind::Polygon => {
                for ring in parse_wkb_polygon_rings(&wkb).unwrap_or_default() {
                    if ring.len() >= 3 {
                        total_vertices += ring.len() as u32;
                        cb.push_feature(&ring, &attr_buf).map_err(io_err)?;
                    }
                }
            }
            GeomKind::LineString => {
                for line in parse_wkb_linestrings(&wkb).unwrap_or_default() {
                    if line.len() >= 2 {
                        total_vertices += line.len() as u32;
                        cb.push_feature(&line, &attr_buf).map_err(io_err)?;
                    }
                }
            }
            GeomKind::Point => {
                if let Some(p) = parse_wkb_point(&wkb) {
                    total_vertices += 1;
                    cb.push_feature(&[p], &attr_buf).map_err(io_err)?;
                }
            }
        }
    }
    let report = VectorCollectionReport {
        name: spec.name.clone(),
        kind: spec.kind,
        feature_count: cb.feature_count(),
        total_vertices,
    };
    Ok((cb, report))
}

fn io_err(e: turbo_tiles_vector::VectorError) -> BuildError {
    BuildError::Logic(format!("vector builder: {e}"))
}

fn encode_attrs(
    specs: &[VectorAttrSpec],
    row: &sqlx::postgres::PgRow,
    buf: &mut [u8],
) -> Result<(), BuildError> {
    let mut offset = 0usize;
    for spec in specs {
        match spec.ty {
            AttrType::F32 => {
                let v: f32 = row
                    .try_get::<Option<f32>, _>(spec.name.as_str())?
                    .unwrap_or(0.0);
                buf[offset..offset + 4].copy_from_slice(&v.to_le_bytes());
                offset += 4;
            }
            AttrType::U32 => {
                let v: i64 = row
                    .try_get::<Option<i64>, _>(spec.name.as_str())?
                    .unwrap_or(0);
                buf[offset..offset + 4].copy_from_slice(&(v as u32).to_le_bytes());
                offset += 4;
            }
            AttrType::U8 => {
                let v: i32 = row
                    .try_get::<Option<i32>, _>(spec.name.as_str())?
                    .unwrap_or(0);
                buf[offset..offset + 1].copy_from_slice(&[v as u8]);
                offset += 1;
            }
        }
    }
    Ok(())
}

// ============================================================================
// Minimal WKB decoders. ST_AsBinary returns little-endian WKB without
// SRID (since we run on ST_Force2D output; the geom type bit is 2D
// only). We handle Polygon (3 = single, 6 = multi), LineString
// (2 = single, 5 = multi), and Point (1 = single, 4 = multi).
// ============================================================================

fn read_u32(buf: &[u8], off: usize) -> Option<u32> {
    if off + 4 > buf.len() {
        return None;
    }
    Some(u32::from_le_bytes([
        buf[off],
        buf[off + 1],
        buf[off + 2],
        buf[off + 3],
    ]))
}
fn read_f64(buf: &[u8], off: usize) -> Option<f64> {
    if off + 8 > buf.len() {
        return None;
    }
    Some(f64::from_le_bytes([
        buf[off],
        buf[off + 1],
        buf[off + 2],
        buf[off + 3],
        buf[off + 4],
        buf[off + 5],
        buf[off + 6],
        buf[off + 7],
    ]))
}

fn parse_wkb_polygon_rings(wkb: &[u8]) -> Option<Vec<Vec<Point>>> {
    let (gt, mut off) = wkb_header(wkb)?;
    match gt {
        3 => Some(read_polygon(wkb, &mut off)),
        6 => {
            let n = read_u32(wkb, off)? as usize;
            off += 4;
            let mut out: Vec<Vec<Point>> = Vec::new();
            for _ in 0..n {
                // each child is itself a polygon-tagged geometry
                let (cgt, coff) = wkb_header(&wkb[off..])?;
                if cgt != 3 {
                    return None;
                }
                let mut absoff = off + coff;
                let rings = read_polygon(wkb, &mut absoff);
                off = absoff;
                out.extend(rings);
            }
            Some(out)
        }
        _ => None,
    }
}

fn parse_wkb_linestrings(wkb: &[u8]) -> Option<Vec<Vec<Point>>> {
    let (gt, mut off) = wkb_header(wkb)?;
    match gt {
        2 => Some(vec![read_linestring(wkb, &mut off)]),
        5 => {
            let n = read_u32(wkb, off)? as usize;
            off += 4;
            let mut out: Vec<Vec<Point>> = Vec::new();
            for _ in 0..n {
                let (cgt, coff) = wkb_header(&wkb[off..])?;
                if cgt != 2 {
                    return None;
                }
                let mut absoff = off + coff;
                let line = read_linestring(wkb, &mut absoff);
                off = absoff;
                out.push(line);
            }
            Some(out)
        }
        _ => None,
    }
}

fn parse_wkb_point(wkb: &[u8]) -> Option<Point> {
    let (gt, off) = wkb_header(wkb)?;
    if gt != 1 {
        return None;
    }
    let x = read_f64(wkb, off)? as f32;
    let y = read_f64(wkb, off + 8)? as f32;
    Some(Point::new(x, y))
}

/// Returns (geom_type_low_16, offset_after_header).
/// Strips both the WKB SRID flag (0x2000_0000) and the Z/M dimension
/// flags (0x8000_0000 / 0x4000_0000). PostGIS `ST_AsBinary` writes
/// pure 2D WKB without SRID, but we tolerate the variants defensively.
fn wkb_header(wkb: &[u8]) -> Option<(u32, usize)> {
    if wkb.len() < 5 || wkb[0] != 1 {
        return None;
    }
    let raw = read_u32(wkb, 1)?;
    let has_srid = (raw & 0x2000_0000) != 0;
    let base = raw & 0xFFFF;
    let off = if has_srid { 9 } else { 5 };
    Some((base, off))
}

fn read_polygon(wkb: &[u8], off: &mut usize) -> Vec<Vec<Point>> {
    let n_rings = match read_u32(wkb, *off) {
        Some(n) => n as usize,
        None => return Vec::new(),
    };
    *off += 4;
    let mut out: Vec<Vec<Point>> = Vec::with_capacity(n_rings);
    for _ in 0..n_rings {
        let n_pts = match read_u32(wkb, *off) {
            Some(n) => n as usize,
            None => return out,
        };
        *off += 4;
        let mut ring: Vec<Point> = Vec::with_capacity(n_pts);
        for _ in 0..n_pts {
            let x = match read_f64(wkb, *off) {
                Some(v) => v as f32,
                None => return out,
            };
            let y = match read_f64(wkb, *off + 8) {
                Some(v) => v as f32,
                None => return out,
            };
            ring.push(Point::new(x, y));
            *off += 16;
        }
        out.push(ring);
    }
    out
}

fn read_linestring(wkb: &[u8], off: &mut usize) -> Vec<Point> {
    let n = match read_u32(wkb, *off) {
        Some(n) => n as usize,
        None => return Vec::new(),
    };
    *off += 4;
    let mut out: Vec<Point> = Vec::with_capacity(n);
    for _ in 0..n {
        let x = match read_f64(wkb, *off) {
            Some(v) => v as f32,
            None => return out,
        };
        let y = match read_f64(wkb, *off + 8) {
            Some(v) => v as f32,
            None => return out,
        };
        out.push(Point::new(x, y));
        *off += 16;
    }
    out
}
