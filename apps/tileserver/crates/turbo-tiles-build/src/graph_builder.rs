//! `norway.graph` builder. Reads `paths.node` + `paths.edge` and
//! writes the CSR graph artifact consumed by `turbo-tiles-graph`.
//!
//! Per-profile cost = Naismith-ish blend of distance + uphill gain.
//! The formula is intentionally simple; a richer cost model lands
//! when slope/aspect/refusal start influencing routing decisions.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use byteorder::{LittleEndian, WriteBytesExt};
use chrono::Utc;
use futures::TryStreamExt;
use serde::Serialize;
use sqlx::Row;
use tracing::info;
use turbo_tiles_artifacts::{write_header, ArtifactKind, Header};
use turbo_tiles_db::DbPool;
use turbo_tiles_graph::{
    write_graph_geom_meta, write_meta, EdgeRecord, GraphGeomIndexEntry, GraphGeomMeta, GraphMeta,
    NodePos, EDGE_RECORD_BYTES, GRAPH_FORMAT_VERSION, GRAPH_GEOM_FORMAT_VERSION,
    GRAPH_GEOM_INDEX_BYTES, PROFILE_COUNT,
};
use turbo_tiles_elev::{Dem, PointXY};

use crate::BuildError;

#[derive(Debug, Clone, Serialize)]
pub struct GraphBuildReport {
    pub out_path: PathBuf,
    pub nodes: u32,
    pub edges_directed: u32,
    pub edges_source_rows: u32,
    pub edges_skipped_no_endpoint: u32,
    pub file_size_bytes: u64,
    pub seconds: f64,
    /// Structural health audit: connectivity by `fkb_type`,
    /// dangling-endpoint count, vertex-density warnings. Carries
    /// the warnings that would have surfaced the session's 49 K
    /// sti-component fragmentation at build time.
    pub health: crate::health::HealthReport,
}

pub async fn build(pool: &DbPool, out_dir: &Path) -> Result<GraphBuildReport, BuildError> {
    let started = Instant::now();
    std::fs::create_dir_all(out_dir)?;

    // If the DEM artifact has already been built, open it so we can
    // compute per-edge slope metrics by sampling elevation along
    // each edge's polyline. Without DEM coverage the metrics stay
    // at zero — graph-side slope cost layer becomes a no-op there.
    let dem_path = out_dir.join("norway.dem");
    let dem: Option<Dem> = if dem_path.exists() {
        match Dem::open(&dem_path) {
            Ok(d) => {
                info!(path = %dem_path.display(), "graph builder will enrich edges with DEM slope metrics");
                Some(d)
            }
            Err(e) => {
                tracing::warn!(error = %e, "DEM artifact present but failed to open; skipping slope enrichment");
                None
            }
        }
    } else {
        None
    };

    // ---- 1. Read all live nodes; compact ids to dense u32 -----------------
    //
    // We read from `paths.edge_vertices_pgr` instead of `paths.node`.
    // The pgRouting topology builder writes new vertices THERE — so
    // when fkb edges' source/target reference vertex ids beyond
    // paths.node's range, we'd miss every fkb edge if we only read
    // paths.node. edge_vertices_pgr is the canonical vertex set:
    // it contains the original turbase vertices PLUS every vertex
    // pgr_createTopology added for the rest of the edge table.
    let mut node_rows = sqlx::query(
        "SELECT id::bigint, ST_X(the_geom)::float8 AS x, ST_Y(the_geom)::float8 AS y \
         FROM paths.edge_vertices_pgr ORDER BY id"
    ).fetch(pool);

    use std::collections::HashMap;
    let mut id_map: HashMap<i64, u32> = HashMap::new();
    let mut nodes: Vec<NodePos> = Vec::new();
    while let Some(row) = node_rows.try_next().await? {
        let id: i64 = row.try_get("id")?;
        let x: f64 = row.try_get("x")?;
        let y: f64 = row.try_get("y")?;
        let new_id = nodes.len() as u32;
        nodes.push(NodePos {
            x: x as f32,
            y: y as f32,
        });
        id_map.insert(id, new_id);
    }
    drop(node_rows);
    info!(nodes = nodes.len(), "loaded nodes");
    if nodes.is_empty() {
        return Err(BuildError::Logic("paths.node empty".into()));
    }

    // ---- 2. Stream live edges; produce undirected pairs -------------------
    // We also pull `geom` so each directed edge can carry its
    // polyline geometry into the sibling `norway.graph_geom`
    // artifact. Without polylines on-graph routes are jagged
    // straight-segment caricatures.
    let mut edge_rows = sqlx::query(
        r#"
        SELECT
          source_node, target_node,
          length_m,
          COALESCE(elevation_gain_m, 0.0) AS gain_m,
          COALESCE(elevation_loss_m, 0.0) AS loss_m,
          fkb_type, marking, surface,
          ingest_source::text AS source_text,
          ST_AsBinary(geom) AS geom_wkb
        FROM paths.edge
        WHERE deleted_at IS NULL
          AND source_node IS NOT NULL
          AND target_node IS NOT NULL
        "#,
    )
    .fetch(pool);

    let mut edges: Vec<EdgeRecord> = Vec::new();
    // Per directed edge, the polyline as (x, y) vertices in EPSG
    // :25833 metres. Built in parallel with `edges` so indices match.
    let mut polylines: Vec<Vec<NodePos>> = Vec::new();
    let mut edges_source_rows: u32 = 0;
    let mut skipped_no_endpoint: u32 = 0;
    while let Some(row) = edge_rows.try_next().await? {
        let src: i64 = row.try_get("source_node")?;
        let dst: i64 = row.try_get("target_node")?;
        let (Some(&u), Some(&v)) = (id_map.get(&src), id_map.get(&dst)) else {
            skipped_no_endpoint += 1;
            continue;
        };
        edges_source_rows += 1;
        let length: f64 = row.try_get("length_m")?;
        let gain: f64 = row.try_get("gain_m")?;
        let loss: f64 = row.try_get("loss_m")?;
        let fkb_type: Option<String> = row.try_get("fkb_type").ok();
        let marking: Option<String> = row.try_get("marking").ok();
        let surface: Option<String> = row.try_get("surface").ok();
        let source_text: Option<String> = row.try_get("source_text").ok();

        let fkb_b = encode_fkb_type(fkb_type.as_deref());
        let marking_b = encode_marking(marking.as_deref());
        let surface_b = encode_surface(surface.as_deref());
        let source_b = encode_source(source_text.as_deref());

        // Decode the WKB LineString. PG returns hex-encoded EWKB
        // for `ST_AsBinary` — actually pure WKB; first byte is
        // byte-order. We only handle little-endian LineString here
        // since that's what 25833-stored PG geometries emit.
        let geom_wkb: Vec<u8> = row.try_get("geom_wkb").unwrap_or_default();
        let mut fwd_pts = parse_wkb_linestring(&geom_wkb).unwrap_or_else(|| {
            // Fall back to a 2-point line between endpoints so the
            // edge still has *some* polyline rather than zero.
            vec![
                nodes[u as usize],
                nodes[v as usize],
            ]
        });
        // Make sure the polyline endpoints match the graph nodes —
        // pgrouting topology snaps edges to nearby nodes, and small
        // (mm-level) deltas exist between the geom and node.geom.
        // Force alignment so downstream geometry-stitching is clean.
        if let Some(first) = fwd_pts.first_mut() {
            *first = nodes[u as usize];
        }
        if let Some(last) = fwd_pts.last_mut() {
            *last = nodes[v as usize];
        }
        let mut rev_pts = fwd_pts.clone();
        rev_pts.reverse();

        // Derive slope metrics by sampling elevation at each
        // polyline vertex. Stored in `slope_max_deg` for the
        // GraphSlopeLayer to read at runtime. When the DEM doesn't
        // cover this edge we leave the field at 0.0 so the layer
        // contributes nothing — honest "no data" instead of guessing.
        let mut slope_max_deg_f: f32 = 0.0;
        let mut slope_max_deg_r: f32 = 0.0;
        let (mut dem_gain_f, mut dem_loss_f) = (0.0_f32, 0.0_f32);
        if let Some(d) = dem.as_ref() {
            let mut sampled: Vec<f32> = Vec::with_capacity(fwd_pts.len());
            for p in &fwd_pts {
                let r = d
                    .sample(PointXY { x: p.x as f64, y: p.y as f64 })
                    .ok()
                    .flatten();
                match r {
                    Some(v) => sampled.push(v),
                    None => {
                        sampled.clear();
                        break;
                    }
                }
            }
            if sampled.len() == fwd_pts.len() && sampled.len() >= 2 {
                for w in fwd_pts.windows(2).zip(sampled.windows(2)) {
                    let (segs, els) = w;
                    let dx = (segs[1].x - segs[0].x) as f64;
                    let dy = (segs[1].y - segs[0].y) as f64;
                    let horiz = (dx * dx + dy * dy).sqrt();
                    if horiz < 1.0 {
                        continue;
                    }
                    let dz = (els[1] - els[0]) as f64;
                    let slope_deg = (dz / horiz).atan().to_degrees().abs() as f32;
                    if slope_deg > slope_max_deg_f {
                        slope_max_deg_f = slope_deg;
                    }
                    if dz > 0.0 {
                        dem_gain_f += dz as f32;
                    } else {
                        dem_loss_f += (-dz) as f32;
                    }
                }
                slope_max_deg_r = slope_max_deg_f; // symmetric
            }
        }
        // Prefer DEM-derived gain/loss when we computed them; fall
        // back to the database row value when the DEM didn't cover.
        let (gain_f, loss_f) = if dem_gain_f > 0.0 || dem_loss_f > 0.0 {
            (dem_gain_f, dem_loss_f)
        } else {
            (gain as f32, loss as f32)
        };

        // Forward direction: (u→v), gain as-is.
        edges.push(EdgeRecord {
            from_id: u,
            to_id: v,
            length_m: length as f32,
            gain_m: gain_f,
            loss_m: loss_f,
            slope_max_deg: slope_max_deg_f,
            fkb_type: fkb_b,
            marking: marking_b,
            surface: surface_b,
            source: source_b,
            attr_flags: 0,
        });
        polylines.push(fwd_pts);
        // Reverse direction: swap gain/loss. Max-slope is symmetric
        // — climbing and descending the same hill have the same
        // peak gradient — so we reuse the same value.
        edges.push(EdgeRecord {
            from_id: v,
            to_id: u,
            length_m: length as f32,
            gain_m: loss_f,
            loss_m: gain_f,
            slope_max_deg: slope_max_deg_r,
            fkb_type: fkb_b,
            marking: marking_b,
            surface: surface_b,
            source: source_b,
            attr_flags: 0,
        });
        polylines.push(rev_pts);
    }
    drop(edge_rows);
    info!(edges = edges.len(), "loaded edges");

    // ---- 3. Sort edges by from_id so CSR offsets are O(E) ------------------
    //
    // Critical: `polylines` is parallel to `edges` (index i in one
    // corresponds to index i in the other). A naive `edges.sort_by_key`
    // would desynchronise them — every per-edge polyline would belong
    // to a different edge, stitching random fragments into the
    // reconstructed route. Sort a permutation, then apply it to both.
    let mut perm: Vec<u32> = (0..edges.len() as u32).collect();
    perm.sort_by_key(|&i| edges[i as usize].from_id);
    let sorted_edges: Vec<EdgeRecord> = perm.iter().map(|&i| edges[i as usize]).collect();
    let sorted_polylines: Vec<Vec<NodePos>> =
        perm.iter().map(|&i| std::mem::take(&mut polylines[i as usize])).collect();
    edges = sorted_edges;
    polylines = sorted_polylines;

    let nc = nodes.len() as u32;
    let ec = edges.len() as u32;
    let mut offsets: Vec<u32> = vec![0; nc as usize + 1];
    for e in &edges {
        offsets[e.from_id as usize + 1] += 1;
    }
    for i in 1..offsets.len() {
        offsets[i] += offsets[i - 1];
    }
    let csr_edges: Vec<u32> = (0..ec).collect();

    // ---- 4. Per-profile cost (Naismith-ish) --------------------------------
    let pc = PROFILE_COUNT;
    let mut costs: Vec<f32> = Vec::with_capacity(ec as usize * pc as usize);
    for e in &edges {
        for profile_id in 0..pc {
            let c = profile_cost(e, profile_id);
            costs.push(c);
        }
    }

    // ---- 5. Write artifact -------------------------------------------------
    let out_path = out_dir.join(ArtifactKind::Graph.filename());
    let tmp_path = out_dir.join(format!("{}.tmp", ArtifactKind::Graph.filename()));
    let f = File::create(&tmp_path)?;
    let mut w = BufWriter::with_capacity(8 * 1024 * 1024, f);
    write_header(
        &mut w,
        &Header {
            kind: ArtifactKind::Graph,
            format_version: GRAPH_FORMAT_VERSION,
            build_timestamp_unix_sec: Utc::now().timestamp(),
        },
    )?;
    write_meta(
        &mut w,
        &GraphMeta {
            node_count: nc,
            edge_count: ec,
            profile_count: pc,
            srid: 25833,
        },
    )?;
    // Nodes (POD)
    w.write_all(bytemuck::cast_slice(&nodes))?;
    // Edges (POD)
    debug_assert_eq!(std::mem::size_of::<EdgeRecord>(), EDGE_RECORD_BYTES);
    w.write_all(bytemuck::cast_slice(&edges))?;
    // CSR offsets
    for o in &offsets {
        w.write_u32::<LittleEndian>(*o)?;
    }
    // CSR edges
    for e in &csr_edges {
        w.write_u32::<LittleEndian>(*e)?;
    }
    // Costs
    for c in &costs {
        w.write_f32::<LittleEndian>(*c)?;
    }
    w.flush()?;
    drop(w);
    std::fs::rename(&tmp_path, &out_path)?;
    let file_size_bytes = std::fs::metadata(&out_path)?.len();

    // ---- 6. Sibling artifact: per-edge polyline geometry ------------------
    //
    // The graph routing core doesn't need geometry to run Dijkstra
    // (only node positions for snap + heuristics) — but
    // reconstructing a route as straight node-to-node segments
    // produces visibly wrong paths through anywhere the underlying
    // trail bends. The polyline artifact carries the original
    // LineString geometry for every directed edge so the API can
    // emit faithful polylines without inflating the routing
    // artifact's hot data.
    //
    // Edge order in the polyline arrays matches the edge order in
    // the graph artifact so the per-edge lookup is O(1).
    let geom_path = out_dir.join(ArtifactKind::GraphGeom.filename());
    let geom_tmp = out_dir.join(format!("{}.tmp", ArtifactKind::GraphGeom.filename()));
    let geom_f = File::create(&geom_tmp)?;
    let mut gw = BufWriter::with_capacity(8 * 1024 * 1024, geom_f);
    let total_verts: u32 = polylines.iter().map(|p| p.len() as u32).sum();
    write_header(
        &mut gw,
        &Header {
            kind: ArtifactKind::GraphGeom,
            format_version: GRAPH_GEOM_FORMAT_VERSION,
            build_timestamp_unix_sec: Utc::now().timestamp(),
        },
    )?;
    write_graph_geom_meta(
        &mut gw,
        &GraphGeomMeta {
            edge_count: ec,
            total_vertices: total_verts,
        },
    )?;
    // Index: (offset, count) per edge.
    let mut offset_acc: u32 = 0;
    let mut index_buf = Vec::with_capacity(ec as usize * GRAPH_GEOM_INDEX_BYTES);
    for poly in &polylines {
        let entry = GraphGeomIndexEntry {
            offset: offset_acc,
            count: poly.len() as u32,
        };
        index_buf.extend_from_slice(bytemuck::bytes_of(&entry));
        offset_acc = offset_acc.saturating_add(poly.len() as u32);
    }
    gw.write_all(&index_buf)?;
    // Flat vertex array.
    for poly in &polylines {
        gw.write_all(bytemuck::cast_slice(poly))?;
    }
    gw.flush()?;
    drop(gw);
    std::fs::rename(&geom_tmp, &geom_path)?;
    let geom_file_size = std::fs::metadata(&geom_path)?.len();
    info!(
        path = %geom_path.display(),
        total_vertices = total_verts,
        file_size_bytes = geom_file_size,
        "graph_geom artifact written"
    );

    // Audit the in-memory graph before we hand it off. The
    // audit's union-find connectivity pass over 1.34M edges costs
    // ~1 s — negligible against the artifact write. The resulting
    // warnings ride along on the build report so anything driving
    // builds (CI, scripts, the SPA) sees them in one place.
    let health = crate::health::audit_graph(&nodes, &edges);
    // Echo high-signal stats / warnings to stderr too — operators
    // running the build CLI shouldn't have to pipe the JSON to
    // catch a sti-fragmentation warning.
    for w in &health.warnings {
        tracing::warn!(code = %w.code, "{}", w.message);
    }
    for e in &health.errors {
        tracing::error!(code = %e.code, "{}", e.message);
    }
    let written_at = chrono::Utc::now().timestamp();
    // Persist the health report alongside the artifact so
    // `verify-artifacts` and CI checks can diff against a baseline
    // without re-running the build.
    let health_path = out_dir.join("norway.graph.health.json");
    let health_body = serde_json::to_vec_pretty(&serde_json::json!({
        "written_at_unix_sec": written_at,
        "report": &health,
    }))
    .unwrap_or_default();
    let _ = std::fs::write(&health_path, &health_body);
    Ok(GraphBuildReport {
        out_path,
        nodes: nc,
        edges_directed: ec,
        edges_source_rows,
        edges_skipped_no_endpoint: skipped_no_endpoint,
        file_size_bytes,
        seconds: started.elapsed().as_secs_f64(),
        health,
    })
}

/// Per-profile cost in "effective metres of forward travel". Naismith
/// distance + uphill gain, times a per-surface multiplier so that
/// `foot` Dijkstra prefers real trails (`sti`) over roads (`vei`),
/// `bicycle` does the opposite, and `ski` favours groomed tracks
/// (`skiloype`). The N50 + Turrutebasen data the builder reads
/// produces only three `fkb_type` values today:
///   1 = sti       (Turrutebasen hiking trails)
///   2 = vei       (N50 roads — paved + gravel + forest + farm)
///   3 = skiloype  (prepared cross-country ski tracks)
/// Anything else encodes to 0 → treated as a road-like default.
fn profile_cost(e: &EdgeRecord, profile_id: u32) -> f32 {
    let gain = e.gain_m.max(0.0);
    let base = match profile_id {
        // Foot — Naismith: every 600 m of vertical = +1 h compared
        // to flat at 5 km/h, i.e. +8 × gain in equivalent distance.
        0 => e.length_m + 8.0 * gain,
        // Bicycle — slightly faster on the flat, much slower uphill.
        1 => e.length_m * 0.6 + 20.0 * gain,
        // Ski — ungroomed default; surface mult favours groomed.
        2 => e.length_m * 1.2 + 6.0 * gain,
        _ => e.length_m,
    };
    base * surface_multiplier(e.fkb_type, profile_id)
}

/// Per (profile, fkb_type) traversal multiplier. Applied on top of
/// the Naismith base cost so that, for example, a foot user routing
/// through 1 km of N50 road sees an effective 1.6 km cost — making
/// Dijkstra naturally prefer a slightly longer trail-based route.
///
/// Values are conservative defaults intended for hiking + cycling
/// + cross-country skiing. The trait-based `CostLayer` system lets
/// callers reweight or veto edges per-request without rebuilding
/// the artifact (see `PreferredEdgeLayer` / `MarkingLayer`).
fn surface_multiplier(fkb_type: u8, profile_id: u32) -> f32 {
    match (profile_id, fkb_type) {
        // FOOT — strongly prefer real trails; tolerate roads but
        // tax them so a 6 km trail beats a 60 km road detour.
        (0, 1) => 1.0, // sti (hiking trail — ideal)
        (0, 2) => 1.6, // vei (road — possible, but punishing on foot)
        (0, 3) => 1.2, // skiloype (open ground in summer)
        (0, _) => 1.4, // unknown — assume road-like

        // BICYCLE — prefer roads, penalise trails + ski tracks.
        (1, 1) => 1.5, // sti (rough for bikes)
        (1, 2) => 1.0, // vei (ideal)
        (1, 3) => 2.0, // skiloype (not really bikeable)
        (1, _) => 1.0,

        // SKI — prepared tracks first, then trails, then roads.
        (2, 1) => 1.2, // sti (skiable but slow)
        (2, 2) => 1.4, // vei (plowed, fast but unrewarding)
        (2, 3) => 1.0, // skiloype (ideal!)
        (2, _) => 1.3,

        _ => 1.0,
    }
}

/// Maps `paths.edge.fkb_type` text to a stable 1-byte code. Values
/// MUST match `surface_multiplier`'s expectations — bump in lockstep.
/// Today the live data carries only three distinct values: `sti`,
/// `vei`, `skiloype`. The older Kartverket variants are still listed
/// so an older ingest can plug back in without re-encoding.
/// Minimal WKB → Vec<NodePos> for a 2D LineString. Handles
/// little-endian byte order and the SRID-prefixed EWKB variant
/// PostGIS emits. Returns `None` on a non-LineString or any
/// structural surprise.
fn parse_wkb_linestring(wkb: &[u8]) -> Option<Vec<NodePos>> {
    if wkb.len() < 9 {
        return None;
    }
    if wkb[0] != 1 {
        // Big-endian is rare from PG; skip rather than handle.
        return None;
    }
    let read_u32 = |off: usize| -> u32 {
        u32::from_le_bytes([wkb[off], wkb[off + 1], wkb[off + 2], wkb[off + 3]])
    };
    let read_f64 = |off: usize| -> f64 {
        f64::from_le_bytes([
            wkb[off], wkb[off + 1], wkb[off + 2], wkb[off + 3],
            wkb[off + 4], wkb[off + 5], wkb[off + 6], wkb[off + 7],
        ])
    };
    let geom_type = read_u32(1);
    let has_srid = (geom_type & 0x2000_0000) != 0;
    let base_type = geom_type & 0xFFFF;
    if base_type != 2 {
        // 2 = LineString. Anything else (Point=1, Polygon=3, …)
        // doesn't belong here.
        return None;
    }
    let mut off = 5;
    if has_srid {
        off += 4;
    }
    if off + 4 > wkb.len() {
        return None;
    }
    let n_points = read_u32(off) as usize;
    off += 4;
    if n_points < 2 {
        return None;
    }
    let mut out: Vec<NodePos> = Vec::with_capacity(n_points);
    for _ in 0..n_points {
        if off + 16 > wkb.len() {
            return None;
        }
        let x = read_f64(off);
        let y = read_f64(off + 8);
        off += 16;
        out.push(NodePos {
            x: x as f32,
            y: y as f32,
        });
    }
    Some(out)
}

fn encode_fkb_type(s: Option<&str>) -> u8 {
    match s.unwrap_or("") {
        "sti" => 1,
        "vei" => 2,
        "skiloype" | "skiløype_preparert" | "lysløype" => 3,
        // Legacy / future surface kinds — map to "trail-ish" by
        // default so they aren't accidentally treated as roads.
        "sti_terreng" => 1,
        "traktorvei" | "skogsvei" => 2,
        _ => 0,
    }
}
fn encode_marking(s: Option<&str>) -> u8 {
    match s.unwrap_or("") {
        "red_t" => 1,
        "cairn" => 2,
        "blue_paint" => 3,
        "unmarked" => 4,
        _ => 0,
    }
}
fn encode_surface(s: Option<&str>) -> u8 {
    match s.unwrap_or("") {
        "natural" => 1,
        "gravel" => 2,
        "asphalt" => 3,
        "boardwalk" => 4,
        _ => 0,
    }
}
fn encode_source(s: Option<&str>) -> u8 {
    match s.unwrap_or("") {
        "fkb" => 1,
        "turbase" => 2,
        "dnt" => 3,
        "manual" => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_edge(from: u32, to: u32) -> EdgeRecord {
        EdgeRecord {
            from_id: from,
            to_id: to,
            length_m: 1.0,
            gain_m: 0.0,
            loss_m: 0.0,
            slope_max_deg: 0.0,
            fkb_type: 0,
            marking: 0,
            surface: 0,
            source: 0,
            attr_flags: 0,
        }
    }

    /// The polylines array must stay in lockstep with edges across
    /// the from_id sort. A naive `edges.sort_by_key` without
    /// permuting `polylines` causes graph_geom to stitch each
    /// routed edge to a stranger's geometry — observed in
    /// production as routes that jump 800 km in a single edge.
    #[test]
    fn polylines_stay_in_lockstep_with_edges_after_sort() {
        let mut edges = vec![mk_edge(5, 0), mk_edge(1, 2), mk_edge(3, 4)];
        let mut polylines: Vec<Vec<NodePos>> = vec![
            vec![NodePos { x: 50.0, y: 50.0 }, NodePos { x: 51.0, y: 51.0 }], // edge (5,0)
            vec![NodePos { x: 10.0, y: 10.0 }, NodePos { x: 11.0, y: 11.0 }], // edge (1,2)
            vec![NodePos { x: 30.0, y: 30.0 }, NodePos { x: 31.0, y: 31.0 }], // edge (3,4)
        ];
        let mut perm: Vec<u32> = (0..edges.len() as u32).collect();
        perm.sort_by_key(|&i| edges[i as usize].from_id);
        let sorted_edges: Vec<EdgeRecord> =
            perm.iter().map(|&i| edges[i as usize]).collect();
        let sorted_polylines: Vec<Vec<NodePos>> = perm
            .iter()
            .map(|&i| std::mem::take(&mut polylines[i as usize]))
            .collect();
        edges = sorted_edges;
        polylines = sorted_polylines;

        // Sorted from_id order is 1, 3, 5.
        assert_eq!(edges[0].from_id, 1);
        assert_eq!(edges[1].from_id, 3);
        assert_eq!(edges[2].from_id, 5);
        // Polyline at i must still describe edges[i]'s geometry.
        // Edge (1,2) → polyline starting at (10,10).
        assert!((polylines[0][0].x - 10.0).abs() < 1e-6);
        // Edge (3,4) → polyline starting at (30,30).
        assert!((polylines[1][0].x - 30.0).abs() < 1e-6);
        // Edge (5,0) → polyline starting at (50,50).
        assert!((polylines[2][0].x - 50.0).abs() < 1e-6);
    }
}
