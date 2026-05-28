//! `norway.mask` builder. Rasterises `terrain.water_polygon` ∪
//! `terrain.glacier_polygon` onto a 100 m grid using a per-polygon
//! scanline algorithm.
//!
//! ## Algorithm
//!
//! 1. Compute the global polygon extent (from a UNION query) and
//!    snap to a 100 m aligned grid.
//! 2. Allocate an in-process `Vec<u8>` of `cells_x * cells_y / 4`
//!    bytes (the eventual packed bitmap) and a parallel
//!    `Vec<RefusalKind>` of `cells_x * cells_y` (working buffer —
//!    cheaper than packing on every polygon contribution).
//! 3. Stream polygons one at a time. For each polygon:
//!    - Parse WKB into `geo::Polygon`.
//!    - Compute the polygon's cell bbox; for each scanline (row),
//!      compute even-odd x-intersections and fill between them.
//! 4. Pack the working buffer to 2 bits per cell and write the
//!    artifact.
//!
//! Memory: working buffer is `cells_x*cells_y` bytes (~200 MB at
//! Norway scale at 100 m). Acceptable on the build host; runtime
//! ingestion reads only the packed 50 MB file.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use futures::TryStreamExt;
use geo::{Coord, LineString, Polygon};
use serde::Serialize;
use sqlx::Row;
use tracing::info;
use turbo_tiles_artifacts::{write_header, ArtifactKind, Header};
use turbo_tiles_db::DbPool;
use turbo_tiles_mask::{
    packed_bytes, write_meta, MaskMeta, RefusalKind, DEFAULT_RESOLUTION_M, MASK_FORMAT_VERSION,
};

use crate::BuildError;

/// Run the post-write mask audit. Reads the just-written
/// artifact, computes coverage stats, optionally cross-references
/// the DEM bbox (from `norway.dem.health.json` if present) to
/// flag bbox mismatches. Writes a sidecar JSON next to the mask
/// for `verify-artifacts` to pick up.
fn audit_and_persist(out_path: &Path) -> crate::health::HealthReport {
    let mask = match turbo_tiles_mask::Mask::open(out_path) {
        Ok(m) => m,
        Err(e) => {
            let mut h = crate::health::HealthReport::default();
            h.error(
                "mask_open_failed",
                format!("audit could not open the just-written mask: {e}"),
                Some("artifact format mismatch — check write/read code parity"),
            );
            return h;
        }
    };
    // Look for a sidecar DEM bbox to cross-check against.
    let dem_bbox = out_path
        .parent()
        .and_then(|d| std::fs::read_to_string(d.join("norway.dem.health.json")).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| {
            let stats = v.pointer("/report/stats")?;
            Some((
                stats.get("dem_min_x")?.as_f64()?,
                stats.get("dem_min_y")?.as_f64()?,
                stats.get("dem_max_x")?.as_f64()?,
                stats.get("dem_max_y")?.as_f64()?,
            ))
        });
    let health = crate::health::audit_mask_coverage(&mask.coverage(), dem_bbox);
    for w in &health.warnings {
        tracing::warn!(code = %w.code, "{}", w.message);
    }
    for e in &health.errors {
        tracing::error!(code = %e.code, "{}", e.message);
    }
    // Sidecar lands next to the mask file (its name varies —
    // norway.mask, norway.wetland.mask, etc — so derive the
    // sidecar name from out_path's stem).
    let sidecar = out_path.with_extension("health.json");
    let body = serde_json::to_vec_pretty(&serde_json::json!({
        "written_at_unix_sec": chrono::Utc::now().timestamp(),
        "report": &health,
    }))
    .unwrap_or_default();
    let _ = std::fs::write(&sidecar, body);
    health
}

#[derive(Debug, Clone, Serialize)]
pub struct MaskBuildReport {
    pub out_path: PathBuf,
    pub cells_x: u32,
    pub cells_y: u32,
    pub water_polygons: u32,
    pub glacier_polygons: u32,
    pub cells_water: u64,
    pub cells_glacier: u64,
    pub file_size_bytes: u64,
    pub seconds: f64,
    /// Coverage stats + bbox-mismatch warnings. See
    /// [`crate::health::audit_mask_coverage`]. The mask audit
    /// optionally cross-references the DEM bbox, which is read
    /// from the sidecar `norway.dem.health.json` if present.
    #[serde(default)]
    pub health: crate::health::HealthReport,
}

/// One source table for `build_from_polygons`. Encapsulates which
/// table to scan, which polygon column to read, and an optional
/// SQL `WHERE` predicate so a single source (e.g. landcover_patch)
/// can be filtered to a specific class without duplicating the
/// scanline-fill code.
///
/// `geom_expression` lets the caller transform the underlying
/// geometry server-side — e.g. `ST_Buffer(senterlinje, 5)` to
/// buffer a stream centerline into a polygon, or
/// `ST_Force2D(geom)` to drop a Z dimension. When `None`,
/// `geom_column` is used as-is.
#[derive(Clone, Copy)]
pub struct PolygonSource {
    pub schema: &'static str,
    pub table: &'static str,
    pub geom_column: &'static str,
    pub geom_expression: Option<&'static str>,
    pub where_clause: Option<&'static str>,
}

/// Build a single-class mask from one or more polygon sources.
/// Output filename is `norway.<name>.mask`. Used for landcover
/// (wetland, forest, built-up, cultivated…) but also any other
/// "is the surface of this type here?" overlay.
///
/// Multiple sources rasterise into the same bitmap — useful when a
/// "developed" mask should union N50's `tettbebyggelse` (built-up
/// area outlines) and `bygning_omrade` (individual buildings).
/// Build a polygon-rasterised mask. `resolution_m` controls cell
/// size — finer values give sharper water/landcover edges but
/// quadruple the artifact size per halving. Caller picks based on
/// the data semantics: water boundaries are critical (use 10–25 m),
/// landcover is broader (50–100 m).
pub async fn build_from_polygons(
    pool: &DbPool,
    out_dir: &Path,
    name: &str,
    sources: &[PolygonSource],
    resolution_m: f32,
) -> Result<MaskBuildReport, BuildError> {
    let started = Instant::now();
    std::fs::create_dir_all(out_dir)?;
    let res = resolution_m as f64;
    let tile_align = res;

    // Compute the global extent across all sources.
    let mut min_x_src = f64::INFINITY;
    let mut max_x_src = f64::NEG_INFINITY;
    let mut min_y_src = f64::INFINITY;
    let mut max_y_src = f64::NEG_INFINITY;
    for src in sources {
        let table = format!("{}.{}", src.schema, src.table);
        let where_sql = src.where_clause.map(|w| format!("WHERE {w}")).unwrap_or_default();
        let geom_expr: String = src
            .geom_expression
            .map(|e| e.to_string())
            .unwrap_or_else(|| src.geom_column.to_string());
        let row = sqlx::query(&format!(
            r#"
            SELECT
              MIN(ST_XMin({geom}))::float8 AS min_x,
              MIN(ST_YMin({geom}))::float8 AS min_y,
              MAX(ST_XMax({geom}))::float8 AS max_x,
              MAX(ST_YMax({geom}))::float8 AS max_y
            FROM {table} {where_sql}
            "#,
            geom = geom_expr,
            table = table,
            where_sql = where_sql
        ))
        .fetch_one(pool)
        .await?;
        let mnx: Option<f64> = row.try_get("min_x").ok();
        let mxx: Option<f64> = row.try_get("max_x").ok();
        let mny: Option<f64> = row.try_get("min_y").ok();
        let mxy: Option<f64> = row.try_get("max_y").ok();
        if let (Some(a), Some(b), Some(c), Some(d)) = (mnx, mxx, mny, mxy) {
            min_x_src = min_x_src.min(a);
            max_x_src = max_x_src.max(b);
            min_y_src = min_y_src.min(c);
            max_y_src = max_y_src.max(d);
        }
    }
    if !min_x_src.is_finite() {
        return Err(BuildError::Logic(format!(
            "no rows for any source of mask '{name}'"
        )));
    }

    let min_x = (min_x_src / tile_align).floor() * tile_align;
    let max_x = (max_x_src / tile_align).ceil() * tile_align;
    let min_y = (min_y_src / tile_align).floor() * tile_align;
    let max_y = (max_y_src / tile_align).ceil() * tile_align;
    let cells_x = ((max_x - min_x) / res).round() as u32;
    let cells_y = ((max_y - min_y) / res).round() as u32;
    let total = cells_x as usize * cells_y as usize;
    let mut cells: Vec<u8> = vec![0u8; total];
    info!(name, cells_x, cells_y, "polygon-mask build starting");

    let mut polygon_count: u32 = 0;
    for src in sources {
        let table = format!("{}.{}", src.schema, src.table);
        let where_sql = src.where_clause.map(|w| format!("WHERE {w}")).unwrap_or_default();
        let geom_expr: String = src
            .geom_expression
            .map(|e| e.to_string())
            .unwrap_or_else(|| src.geom_column.to_string());
        let sql = format!(
            "SELECT ST_AsBinary((ST_Dump({geom})).geom) AS wkb \
             FROM {table} {where_sql}",
            geom = geom_expr,
            table = table,
            where_sql = where_sql
        );
        let mut rows = sqlx::query(&sql).fetch(pool);
        while let Some(row) = rows.try_next().await? {
            let wkb: Vec<u8> = row.try_get("wkb")?;
            let Some(poly) = parse_wkb_polygon(&wkb) else {
                continue;
            };
            polygon_count += 1;
            scanline_fill(&poly, 1u8, &mut cells, cells_x, cells_y, min_x, max_y, res);
            if polygon_count % 50_000 == 0 {
                info!(name, polygon_count, "polygon-mask progress");
            }
        }
        drop(rows);
    }

    let n_packed = packed_bytes(total as u64) as usize;
    let mut packed = vec![0u8; n_packed];
    let mut present_cells = 0u64;
    for (i, &v) in cells.iter().enumerate() {
        if v == 1 {
            present_cells += 1;
        }
        let byte_i = i / 4;
        let bit_off = (i % 4) * 2;
        packed[byte_i] |= (v & 0b11) << bit_off;
    }

    let filename = format!("norway.{name}.mask");
    let out_path = out_dir.join(&filename);
    let tmp_path = out_dir.join(format!("{}.tmp", filename));
    let f = File::create(&tmp_path)?;
    let mut w = BufWriter::with_capacity(8 * 1024 * 1024, f);
    write_header(
        &mut w,
        &Header {
            kind: ArtifactKind::Mask,
            format_version: MASK_FORMAT_VERSION,
            build_timestamp_unix_sec: Utc::now().timestamp(),
        },
    )?;
    write_meta(
        &mut w,
        &MaskMeta {
            min_x,
            min_y,
            max_x,
            max_y,
            cells_x,
            cells_y,
            resolution_m,
        },
    )?;
    w.write_all(&packed)?;
    w.flush()?;
    drop(w);
    std::fs::rename(&tmp_path, &out_path)?;
    let file_size_bytes = std::fs::metadata(&out_path)?.len();
    info!(
        name,
        polygon_count, present_cells, file_size_bytes, "polygon-mask built"
    );
    let health = audit_and_persist(&out_path);
    Ok(MaskBuildReport {
        out_path,
        cells_x,
        cells_y,
        water_polygons: polygon_count,
        glacier_polygons: 0,
        cells_water: present_cells,
        cells_glacier: 0,
        file_size_bytes,
        seconds: started.elapsed().as_secs_f64(),
        health,
    })
}

/// Build a single-class landcover mask (e.g. wetland, forest). Uses
/// the same on-disk format as `norway.mask` — readers don't need to
/// know they're getting a different class. The `class` argument
/// selects `terrain.landcover_patch.class = 'wetland'` (etc.) and
/// emits `norway.<class>.mask`.
pub async fn build_landcover(
    pool: &DbPool,
    out_dir: &Path,
    class: &str,
) -> Result<MaskBuildReport, BuildError> {
    let started = Instant::now();
    std::fs::create_dir_all(out_dir)?;
    let res = DEFAULT_RESOLUTION_M as f64;
    let tile_align = res;

    let ext = sqlx::query(
        &format!(
            r#"
            SELECT
              MIN(ST_XMin(geom))::float8 AS min_x,
              MIN(ST_YMin(geom))::float8 AS min_y,
              MAX(ST_XMax(geom))::float8 AS max_x,
              MAX(ST_YMax(geom))::float8 AS max_y
            FROM terrain.landcover_patch
            WHERE class = $1
            "#
        ),
    )
    .bind(class)
    .fetch_one(pool)
    .await?;
    let min_x_src: Option<f64> = ext.try_get("min_x").ok();
    let max_x_src: Option<f64> = ext.try_get("max_x").ok();
    let min_y_src: Option<f64> = ext.try_get("min_y").ok();
    let max_y_src: Option<f64> = ext.try_get("max_y").ok();
    let (min_x_src, max_x_src, min_y_src, max_y_src) = match (min_x_src, max_x_src, min_y_src, max_y_src) {
        (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
        _ => {
            return Err(BuildError::Logic(format!(
                "no terrain.landcover_patch rows for class='{class}'"
            )))
        }
    };
    let min_x = (min_x_src / tile_align).floor() * tile_align;
    let max_x = (max_x_src / tile_align).ceil() * tile_align;
    let min_y = (min_y_src / tile_align).floor() * tile_align;
    let max_y = (max_y_src / tile_align).ceil() * tile_align;
    let cells_x = ((max_x - min_x) / res).round() as u32;
    let cells_y = ((max_y - min_y) / res).round() as u32;
    let total = cells_x as usize * cells_y as usize;
    let mut cells: Vec<u8> = vec![0u8; total];

    // Stream + scanline-fill — same code path as the water/glacier
    // mask. Filtering by class keeps the SQL streaming and only
    // pulls the rows we need.
    let sql = format!(
        "SELECT ST_AsBinary((ST_Dump(geom)).geom) AS wkb \
         FROM terrain.landcover_patch WHERE class = $1"
    );
    let mut rows = sqlx::query(&sql).bind(class).fetch(pool);
    let mut polygon_count: u32 = 0;
    while let Some(row) = rows.try_next().await? {
        let wkb: Vec<u8> = row.try_get("wkb")?;
        let Some(poly) = parse_wkb_polygon(&wkb) else {
            continue;
        };
        polygon_count += 1;
        scanline_fill(&poly, 1u8, &mut cells, cells_x, cells_y, min_x, max_y, res);
        if polygon_count % 50_000 == 0 {
            info!(class, polygon_count, "landcover progress");
        }
    }
    drop(rows);

    let n_packed = packed_bytes(total as u64) as usize;
    let mut packed = vec![0u8; n_packed];
    let mut present_cells = 0u64;
    for (i, &v) in cells.iter().enumerate() {
        if v == 1 {
            present_cells += 1;
        }
        let byte_i = i / 4;
        let bit_off = (i % 4) * 2;
        packed[byte_i] |= (v & 0b11) << bit_off;
    }

    let filename = format!("norway.{class}.mask");
    let out_path = out_dir.join(&filename);
    let tmp_path = out_dir.join(format!("{}.tmp", filename));
    let f = File::create(&tmp_path)?;
    let mut w = BufWriter::with_capacity(8 * 1024 * 1024, f);
    write_header(
        &mut w,
        &Header {
            kind: ArtifactKind::Mask,
            format_version: MASK_FORMAT_VERSION,
            build_timestamp_unix_sec: Utc::now().timestamp(),
        },
    )?;
    write_meta(
        &mut w,
        &MaskMeta {
            min_x,
            min_y,
            max_x,
            max_y,
            cells_x,
            cells_y,
            resolution_m: DEFAULT_RESOLUTION_M,
        },
    )?;
    w.write_all(&packed)?;
    w.flush()?;
    drop(w);
    std::fs::rename(&tmp_path, &out_path)?;
    let file_size_bytes = std::fs::metadata(&out_path)?.len();

    info!(class, polygon_count, present_cells, file_size_bytes, "landcover mask built");
    let health = audit_and_persist(&out_path);
    Ok(MaskBuildReport {
        out_path,
        cells_x,
        cells_y,
        water_polygons: polygon_count,
        glacier_polygons: 0,
        cells_water: present_cells,
        cells_glacier: 0,
        file_size_bytes,
        seconds: started.elapsed().as_secs_f64(),
        health,
    })
}

pub async fn build(pool: &DbPool, out_dir: &Path) -> Result<MaskBuildReport, BuildError> {
    let started = Instant::now();
    std::fs::create_dir_all(out_dir)?;
    // Water + glacier are the dominant hard-refusal surfaces — a
    // 100-m rasterisation puffs every lake outline by ½ a cell on
    // each side, which the off-trail solver then treats as ~50 m
    // of impassable buffer around shorelines. Drop to 25 m so
    // lake edges, river polygons, and small ponds are accurate to
    // ~12.5 m. ~16× the file size; still under a GB.
    let resolution_m: f32 = 25.0;
    let res = resolution_m as f64;
    let tile_align = res;

    // ---- 1. Extent across both polygon sources ------------------------------
    let ext_row = sqlx::query(
        r#"
        WITH p AS (
          SELECT geom FROM terrain.water_polygon
          UNION ALL
          SELECT geom FROM terrain.glacier_polygon
        )
        SELECT
          MIN(ST_XMin(geom))::float8 AS min_x,
          MIN(ST_YMin(geom))::float8 AS min_y,
          MAX(ST_XMax(geom))::float8 AS max_x,
          MAX(ST_YMax(geom))::float8 AS max_y
        FROM p
        "#,
    )
    .fetch_one(pool)
    .await?;
    let min_x_src: Option<f64> = ext_row.try_get("min_x").ok();
    let max_x_src: Option<f64> = ext_row.try_get("max_x").ok();
    let min_y_src: Option<f64> = ext_row.try_get("min_y").ok();
    let max_y_src: Option<f64> = ext_row.try_get("max_y").ok();
    let (min_x_src, max_x_src, min_y_src, max_y_src) =
        match (min_x_src, max_x_src, min_y_src, max_y_src) {
            (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
            _ => return Err(BuildError::Logic("no polygons in terrain.* tables".into())),
        };
    let min_x = (min_x_src / tile_align).floor() * tile_align;
    let max_x = (max_x_src / tile_align).ceil() * tile_align;
    let min_y = (min_y_src / tile_align).floor() * tile_align;
    let max_y = (max_y_src / tile_align).ceil() * tile_align;
    let cells_x = ((max_x - min_x) / res).round() as u32;
    let cells_y = ((max_y - min_y) / res).round() as u32;
    if cells_x == 0 || cells_y == 0 {
        return Err(BuildError::Logic("empty mask grid".into()));
    }
    info!(cells_x, cells_y, ?min_x, ?min_y, ?max_x, ?max_y, "computed mask grid");

    // ---- 2. Working buffer (one byte per cell) ------------------------------
    let total = cells_x as usize * cells_y as usize;
    let mut cells: Vec<u8> = vec![0u8; total];

    // ---- 3. Rasterise water polygons ----------------------------------------
    let water_polygons = rasterise_layer(
        pool,
        "terrain.water_polygon",
        RefusalKind::Water,
        &mut cells,
        cells_x,
        cells_y,
        min_x,
        max_y,
        res,
    )
    .await?;

    // ---- 4. Rasterise glacier polygons (overrides water on overlap) --------
    let glacier_polygons = rasterise_layer(
        pool,
        "terrain.glacier_polygon",
        RefusalKind::Glacier,
        &mut cells,
        cells_x,
        cells_y,
        min_x,
        max_y,
        res,
    )
    .await?;

    // ---- 5. Pack + write ---------------------------------------------------
    let mut cells_water = 0u64;
    let mut cells_glacier = 0u64;
    let n_packed = packed_bytes(total as u64) as usize;
    let mut packed = vec![0u8; n_packed];
    for (i, &v) in cells.iter().enumerate() {
        match v {
            1 => cells_water += 1,
            2 => cells_glacier += 1,
            _ => {}
        }
        let byte_i = i / 4;
        let bit_off = (i % 4) * 2;
        packed[byte_i] |= (v & 0b11) << bit_off;
    }

    let out_path = out_dir.join(ArtifactKind::Mask.filename());
    let tmp_path = out_dir.join(format!("{}.tmp", ArtifactKind::Mask.filename()));
    let f = File::create(&tmp_path)?;
    let mut w = BufWriter::with_capacity(8 * 1024 * 1024, f);
    write_header(
        &mut w,
        &Header {
            kind: ArtifactKind::Mask,
            format_version: MASK_FORMAT_VERSION,
            build_timestamp_unix_sec: Utc::now().timestamp(),
        },
    )?;
    write_meta(
        &mut w,
        &MaskMeta {
            min_x,
            min_y,
            max_x,
            max_y,
            cells_x,
            cells_y,
            resolution_m,
        },
    )?;
    w.write_all(&packed)?;
    w.flush()?;
    drop(w);
    std::fs::rename(&tmp_path, &out_path)?;
    let file_size_bytes = std::fs::metadata(&out_path)?.len();

    let health = audit_and_persist(&out_path);
    Ok(MaskBuildReport {
        out_path,
        cells_x,
        cells_y,
        water_polygons,
        glacier_polygons,
        cells_water,
        cells_glacier,
        file_size_bytes,
        seconds: started.elapsed().as_secs_f64(),
        health,
    })
}

#[allow(clippy::too_many_arguments)]
async fn rasterise_layer(
    pool: &DbPool,
    table: &str,
    value: RefusalKind,
    cells: &mut [u8],
    cells_x: u32,
    cells_y: u32,
    min_x: f64,
    max_y: f64,
    res: f64,
) -> Result<u32, BuildError> {
    // Stream the polygons as WKB via ST_AsBinary, after exploding
    // MultiPolygons via ST_Dump so each row is a single polygon.
    let sql = format!(
        "SELECT ST_AsBinary((ST_Dump(geom)).geom) AS wkb FROM {table}"
    );
    let mut rows = sqlx::query(&sql).fetch(pool);
    let mut polygon_count: u32 = 0;
    let v = value as u8;
    while let Some(row) = rows.try_next().await? {
        let wkb: Vec<u8> = row.try_get("wkb")?;
        let Some(poly) = parse_wkb_polygon(&wkb) else {
            continue;
        };
        polygon_count += 1;
        scanline_fill(&poly, v, cells, cells_x, cells_y, min_x, max_y, res);
        if polygon_count % 10_000 == 0 {
            info!(table, polygon_count, "mask layer progress");
        }
    }
    Ok(polygon_count)
}

/// Minimal WKB → geo::Polygon parser. Only handles the
/// little-endian Polygon geometry kind (1) — the only thing
/// `ST_Dump` will hand us.
fn parse_wkb_polygon(wkb: &[u8]) -> Option<Polygon<f64>> {
    if wkb.len() < 9 {
        return None;
    }
    let byte_order = wkb[0];
    if byte_order != 1 {
        // Big-endian WKB is rare from PG; skip.
        return None;
    }
    let read_u32 = |off: usize| -> u32 {
        u32::from_le_bytes([wkb[off], wkb[off + 1], wkb[off + 2], wkb[off + 3]])
    };
    let read_f64 = |off: usize| -> f64 {
        f64::from_le_bytes([
            wkb[off],
            wkb[off + 1],
            wkb[off + 2],
            wkb[off + 3],
            wkb[off + 4],
            wkb[off + 5],
            wkb[off + 6],
            wkb[off + 7],
        ])
    };
    let geom_type = read_u32(1);
    // 3 = Polygon; 0x20000000 bit set means SRID-prefixed (EWKB).
    let has_srid = (geom_type & 0x2000_0000) != 0;
    let base_type = geom_type & 0xFFFF;
    if base_type != 3 {
        return None;
    }
    let mut off = 5;
    if has_srid {
        off += 4; // skip SRID
    }
    let num_rings = read_u32(off) as usize;
    off += 4;
    if num_rings == 0 {
        return None;
    }
    let mut rings: Vec<Vec<Coord<f64>>> = Vec::with_capacity(num_rings);
    for _ in 0..num_rings {
        if off + 4 > wkb.len() {
            return None;
        }
        let n_points = read_u32(off) as usize;
        off += 4;
        let mut ring = Vec::with_capacity(n_points);
        for _ in 0..n_points {
            if off + 16 > wkb.len() {
                return None;
            }
            let x = read_f64(off);
            let y = read_f64(off + 8);
            off += 16;
            ring.push(Coord { x, y });
        }
        rings.push(ring);
    }
    let exterior = LineString::from(rings.remove(0));
    let interiors: Vec<LineString<f64>> =
        rings.into_iter().map(LineString::from).collect();
    Some(Polygon::new(exterior, interiors))
}

/// Scanline-fill polygon `poly` into the `cells` buffer using value
/// `v`. Even-odd rule over horizontal scanlines aligned with the
/// output grid. Last-write-wins: glacier overlay on top of water is
/// naturally handled by calling order.
#[allow(clippy::too_many_arguments)]
fn scanline_fill(
    poly: &Polygon<f64>,
    v: u8,
    cells: &mut [u8],
    cells_x: u32,
    cells_y: u32,
    min_x: f64,
    max_y: f64,
    res: f64,
) {
    // Polygon bbox in cell coords.
    let mut p_min_x = f64::INFINITY;
    let mut p_max_x = f64::NEG_INFINITY;
    let mut p_min_y = f64::INFINITY;
    let mut p_max_y = f64::NEG_INFINITY;
    for c in poly.exterior().0.iter() {
        if c.x < p_min_x {
            p_min_x = c.x;
        }
        if c.x > p_max_x {
            p_max_x = c.x;
        }
        if c.y < p_min_y {
            p_min_y = c.y;
        }
        if c.y > p_max_y {
            p_max_y = c.y;
        }
    }
    let col_min = (((p_min_x - min_x) / res).floor() as i64).max(0);
    let col_max = (((p_max_x - min_x) / res).ceil() as i64).min(cells_x as i64 - 1);
    let row_min = (((max_y - p_max_y) / res).floor() as i64).max(0);
    let row_max = (((max_y - p_min_y) / res).ceil() as i64).min(cells_y as i64 - 1);
    if col_min > col_max || row_min > row_max {
        return;
    }
    // Collect all rings (exterior + interiors). Even-odd rule
    // naturally handles holes when we count all crossings together.
    let mut all_segments: Vec<(Coord<f64>, Coord<f64>)> = Vec::new();
    for ls in std::iter::once(poly.exterior()).chain(poly.interiors().iter()) {
        let coords = &ls.0;
        for w in coords.windows(2) {
            all_segments.push((w[0], w[1]));
        }
    }
    for row in row_min..=row_max {
        // Scanline y = world y of cell-row centre. Cells span
        // [max_y - (row+1)*res, max_y - row*res] in world Y.
        let y = max_y - (row as f64 + 0.5) * res;
        let mut crossings: Vec<f64> = Vec::new();
        for &(a, b) in &all_segments {
            // Skip horizontal edges (would yield infinite crossings
            // and contribute zero crossings on the even-odd rule).
            if (a.y > y) == (b.y > y) {
                continue;
            }
            // Compute X of intersection between edge and scanline.
            let t = (y - a.y) / (b.y - a.y);
            let x = a.x + t * (b.x - a.x);
            crossings.push(x);
        }
        crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut i = 0;
        while i + 1 < crossings.len() {
            let x0 = crossings[i];
            let x1 = crossings[i + 1];
            let c0 = (((x0 - min_x) / res).floor() as i64).max(col_min);
            let c1 = (((x1 - min_x) / res).ceil() as i64).min(col_max);
            if c0 <= c1 {
                let base = row as usize * cells_x as usize;
                for c in c0..=c1 {
                    cells[base + c as usize] = v;
                }
            }
            i += 2;
        }
    }
}
