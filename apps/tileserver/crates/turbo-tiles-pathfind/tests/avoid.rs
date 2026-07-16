//! Behavioural tests for "Avoid Marked" + round-trip self-avoidance
//! (spec Phase 5). Deterministic, in-process, on synthetic graphs — no
//! server, no production artifacts.
//!
//! The invariant under test is the product objection the spec calls out:
//! avoidance must penalise the TRAIL EDGES an avoided geometry runs
//! along (a graph-edge multiplier), NOT buffer the mesh cells near it.
//! An edge penalty makes a divergent marked trail the cheapest escape;
//! a spatial buffer would instead let the route "shadow-walk" ~radius
//! metres off-trail, parallel to the corridor, forever. The
//! `no_shadow_walking` test fails loudly if the router ever does that.
#![allow(dead_code, clippy::field_reassign_with_default)]

use std::io::Write;
use std::sync::Arc;

use byteorder::{LittleEndian, WriteBytesExt};
use turbo_tiles_artifacts::{write_header as write_art_header, ArtifactKind, Header};
use turbo_tiles_graph::{
    write_meta as write_graph_meta, EdgeRecord, Graph, GraphMeta, NodePos, GRAPH_FORMAT_VERSION,
};
use turbo_tiles_pathfind::{utm33n_to_wgs84, Path, Pathfinder, Prefs};

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// Flat DEM (constant elevation) covering a `cells × cells` grid at 10 m
/// resolution from upper-left `(ulx, uly)`. Flat terrain reduces the
/// off-trail mesh to the straight-line geodesic, so the tests isolate
/// the TRAIL-vs-off-trail cost decision the avoidance feature drives.
fn write_flat_dem(path: &std::path::Path, ulx: f64, uly: f64, cells: u32, elev: f32) {
    use turbo_tiles_artifacts::HEADER_BYTES;
    use turbo_tiles_elev::{
        write_meta as write_dem_meta, write_tile_entry, DemMeta, TileEntry, COMPRESSION_ZSTD,
        DEM_FORMAT_VERSION, DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
    };
    let meta = DemMeta {
        tile_count: 1,
        tile_cells: cells,
        pixel_size_m: 10.0,
        nodata: NODATA_SENTINEL,
        compression: COMPRESSION_ZSTD,
    };
    let mut f = std::fs::File::create(path).unwrap();
    write_art_header(
        &mut f,
        &Header {
            kind: ArtifactKind::Dem,
            format_version: DEM_FORMAT_VERSION,
            build_timestamp_unix_sec: 0,
        },
    )
    .unwrap();
    write_dem_meta(&mut f, &meta).unwrap();
    let data = vec![elev; (cells * cells) as usize];
    let compressed = zstd::encode_all(bytemuck::cast_slice::<f32, u8>(&data), 1).unwrap();
    let dir_offset = (HEADER_BYTES + DEM_META_BYTES) as u64;
    let payload_offset = dir_offset + TILE_ENTRY_BYTES as u64;
    let entry = TileEntry {
        ulx,
        uly,
        offset: payload_offset,
        compressed_size: compressed.len() as u32,
    };
    write_tile_entry(&mut f, &entry).unwrap();
    f.write_all(&compressed).unwrap();
    f.sync_all().unwrap();
}

/// Flat DEM (elev 100 m) covering ~6 km around a test origin.
fn flat_dem_around(ox: f64, oy: f64) -> (tempfile::NamedTempFile, Arc<turbo_tiles_elev::Dem>) {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_flat_dem(tmp.path(), ox - 2000.0, oy + 2000.0, 600, 100.0);
    let dem = Arc::new(turbo_tiles_elev::Dem::open(tmp.path()).unwrap());
    (tmp, dem)
}

/// A marked trail edge (`fkb_type = sti`, red-T marked) of the given
/// straight-line length.
fn trail_edge(from: u32, to: u32, len_m: f32) -> EdgeRecord {
    EdgeRecord {
        from_id: from,
        to_id: to,
        length_m: len_m,
        gain_m: 0.0,
        loss_m: 0.0,
        slope_max_deg: 0.0,
        fkb_type: 1, // sti / marked trail
        marking: 1,  // red T
        surface: 0,
        source: 0,
        attr_flags: 0,
    }
}

/// Write a routing-graph artifact for arbitrary nodes + directed edges.
/// Edges are sorted by `from_id` so the identity CSR edge list is valid.
fn write_graph(path: &std::path::Path, nodes: &[NodePos], edges: &[EdgeRecord]) {
    let mut edges = edges.to_vec();
    edges.sort_by_key(|e| e.from_id);
    let mut f = std::fs::File::create(path).unwrap();
    let nc = nodes.len() as u32;
    let ec = edges.len() as u32;
    let pc = 3u32;
    write_art_header(
        &mut f,
        &Header {
            kind: ArtifactKind::Graph,
            format_version: GRAPH_FORMAT_VERSION,
            build_timestamp_unix_sec: 0,
        },
    )
    .unwrap();
    write_graph_meta(
        &mut f,
        &GraphMeta {
            node_count: nc,
            edge_count: ec,
            profile_count: pc,
            srid: 25833,
        },
    )
    .unwrap();
    f.write_all(bytemuck::cast_slice(nodes)).unwrap();
    f.write_all(bytemuck::cast_slice(&edges)).unwrap();
    let mut offsets = vec![0u32; (nc as usize) + 1];
    for e in &edges {
        offsets[e.from_id as usize + 1] += 1;
    }
    for i in 1..offsets.len() {
        offsets[i] += offsets[i - 1];
    }
    for o in &offsets {
        f.write_u32::<LittleEndian>(*o).unwrap();
    }
    for i in 0..ec {
        f.write_u32::<LittleEndian>(i).unwrap();
    }
    for e in &edges {
        for _ in 0..pc {
            f.write_f32::<LittleEndian>(e.length_m).unwrap();
        }
    }
    f.sync_all().unwrap();
}

/// A bidirectional marked-trail edge pair between two nodes, length =
/// their planar distance.
fn bidir(nodes: &[NodePos], a: u32, b: u32) -> Vec<EdgeRecord> {
    let pa = nodes[a as usize];
    let pb = nodes[b as usize];
    let len = ((pb.x - pa.x).powi(2) + (pb.y - pa.y).powi(2)).sqrt();
    vec![trail_edge(a, b, len), trail_edge(b, a, len)]
}

// ---------------------------------------------------------------------
// Geometry helpers (all in LOCAL metres relative to the scene anchor)
// ---------------------------------------------------------------------

/// A scene anchored at a real UTM33N origin so the WGS84 round-trip and
/// DEM coverage line up. Local (dx, dy) metres map to lon/lat via the
/// anchor.
struct Scene {
    ox: f64,
    oy: f64,
    _dem_tmp: tempfile::NamedTempFile,
    _graph_tmp: tempfile::NamedTempFile,
    pf: Pathfinder,
}

impl Scene {
    /// Build a scene from local node positions `(dx, dy)` and directed
    /// edges (node-index pairs, made bidirectional).
    fn new(local_nodes: &[(f64, f64)], edge_pairs: &[(u32, u32)]) -> Self {
        let anchor = turbo_tiles_elev::wgs84_to_utm33n(10.7522, 59.9139);
        let (ox, oy) = (anchor.x, anchor.y);
        let nodes: Vec<NodePos> = local_nodes
            .iter()
            .map(|&(dx, dy)| NodePos {
                x: (ox + dx) as f32,
                y: (oy + dy) as f32,
            })
            .collect();
        let mut edges: Vec<EdgeRecord> = Vec::new();
        for &(a, b) in edge_pairs {
            edges.extend(bidir(&nodes, a, b));
        }
        let graph_tmp = tempfile::NamedTempFile::new().unwrap();
        write_graph(graph_tmp.path(), &nodes, &edges);
        let g = Graph::open(graph_tmp.path()).unwrap();
        let (dem_tmp, dem) = flat_dem_around(ox, oy);
        let pf = Pathfinder::with_defaults(Some(dem), None, Some(Arc::new(g)));
        Self {
            ox,
            oy,
            _dem_tmp: dem_tmp,
            _graph_tmp: graph_tmp,
            pf,
        }
    }

    /// Local metres → request `[lon, lat]`.
    fn ll(&self, dx: f64, dy: f64) -> [f64; 2] {
        let (lon, lat) = utm33n_to_wgs84(self.ox + dx, self.oy + dy);
        [lon, lat]
    }

    /// A local polyline → an avoid polyline in `[lon, lat]`.
    fn avoid_line(&self, pts: &[(f64, f64)]) -> Vec<[f64; 2]> {
        pts.iter().map(|&(dx, dy)| self.ll(dx, dy)).collect()
    }

    /// Route geometry projected back to LOCAL metres `(dx, dy)`.
    fn local_geom(&self, path: &Path) -> Vec<(f64, f64)> {
        path.geometry
            .iter()
            .map(|p| {
                let u = turbo_tiles_elev::wgs84_to_utm33n(p[0], p[1]);
                (u.x - self.ox, u.y - self.oy)
            })
            .collect()
    }
}

/// Fraction (0..1) of a polyline's LENGTH that lies within `radius` of a
/// reference polyline — the overlap metric the spec's thresholds use.
fn overlap_fraction(geom: &[(f64, f64)], reference: &[(f64, f64)], radius: f64) -> f64 {
    if geom.len() < 2 || reference.len() < 2 {
        return 0.0;
    }
    let r2 = radius * radius;
    let mut within = 0.0;
    let mut total = 0.0;
    for w in geom.windows(2) {
        let (ax, ay) = w[0];
        let (bx, by) = w[1];
        let seg = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
        if seg <= 0.0 {
            continue;
        }
        let steps = ((seg / (radius.max(1.0))).ceil() as usize).max(1);
        let w_m = seg / steps as f64;
        for s in 0..steps {
            let t = (s as f64 + 0.5) / steps as f64;
            let px = ax + (bx - ax) * t;
            let py = ay + (by - ay) * t;
            total += w_m;
            if min_dist2_to_polyline(px, py, reference) <= r2 {
                within += w_m;
            }
        }
    }
    if total > 0.0 {
        within / total
    } else {
        0.0
    }
}

fn min_dist2_to_polyline(px: f64, py: f64, poly: &[(f64, f64)]) -> f64 {
    let mut best = f64::INFINITY;
    for w in poly.windows(2) {
        best = best.min(point_seg_dist2(px, py, w[0], w[1]));
    }
    best
}

fn point_seg_dist2(px: f64, py: f64, a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    if len2 <= 0.0 {
        return (px - a.0).powi(2) + (py - a.1).powi(2);
    }
    let t = (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0);
    let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
    (px - cx).powi(2) + (py - cy).powi(2)
}

/// Metres of route in the `off_trail` surface bucket.
fn off_trail_m(path: &Path) -> f64 {
    path.fkb_breakdown.get("off_trail").copied().unwrap_or(0.0)
}

/// Min local distance from any route vertex to a local point.
fn min_dist_to_point(geom: &[(f64, f64)], p: (f64, f64)) -> f64 {
    geom.iter()
        .map(|&(x, y)| ((x - p.0).powi(2) + (y - p.1).powi(2)).sqrt())
        .fold(f64::INFINITY, f64::min)
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

/// Strong trail preference for the synthetic FLAT-DEM fixtures.
///
/// On flat ground the `trail_proximity` bonus makes a mesh cell sitting
/// ON a trail cheaper than the trail edge itself, so the solver would
/// ride trails as off-trail mesh and the edge-based avoid penalty (which
/// only touches graph edges) would never bite. On real terrain, slope +
/// roughness make trail EDGES the cheap line — the regime the spec's
/// model assumes ("off-trail base cost stays high"). We pin a high
/// `off_trail_base` here to reproduce that regime deterministically
/// (same spirit as `pathfinder.rs` pinning `off_trail_base` for
/// calibration-independence).
const TRAIL_PREF_OFF_TRAIL_BASE: f64 = 12.0;

fn base_prefs() -> Prefs {
    let mut p = Prefs::default();
    p.off_trail_base = Some(TRAIL_PREF_OFF_TRAIL_BASE);
    p
}

#[test]
fn detour_when_possible() {
    // Direct trail A→Mid→B along y=0, plus a divergent trail A→P→B that
    // bulges 400 m north. Avoid the direct path. The route must take the
    // divergent trail, so its overlap with the avoided corridor is low.
    // Nodes: 0=A(0,0) 1=Mid(500,0) 2=B(1000,0) 3=P(500,400)
    let scene = Scene::new(
        &[(0.0, 0.0), (500.0, 0.0), (1000.0, 0.0), (500.0, 400.0)],
        &[(0, 1), (1, 2), (0, 3), (3, 2)],
    );
    let mut prefs = base_prefs();
    prefs.avoid = vec![scene.avoid_line(&[(0.0, 0.0), (500.0, 0.0), (1000.0, 0.0)])];

    let path = scene
        .pf
        .solve(scene.ll(0.0, 0.0), scene.ll(1000.0, 0.0), prefs)
        .expect("a detour route exists");
    let geom = scene.local_geom(&path);

    // Baseline: with NO avoidance the router takes the direct trail
    // (~fully overlapping y=0). This proves the avoid actually moved it.
    let direct = scene
        .pf
        .solve(scene.ll(0.0, 0.0), scene.ll(1000.0, 0.0), base_prefs())
        .unwrap();
    let direct_overlap = overlap_fraction(
        &scene.local_geom(&direct),
        &[(0.0, 0.0), (1000.0, 0.0)],
        30.0,
    );
    assert!(
        direct_overlap > 0.8,
        "sanity: un-avoided route should hug the direct path (overlap {direct_overlap:.2})"
    );

    let overlap = overlap_fraction(&geom, &[(0.0, 0.0), (1000.0, 0.0)], 30.0);
    assert!(
        overlap < 0.4,
        "avoided route should detour off the direct corridor (overlap {overlap:.2})"
    );
    // And it detoured via the real alternative trail, not off-trail.
    assert!(
        off_trail_m(&path) < 60.0,
        "detour should stay on the divergent trail (off_trail {:.0} m)",
        off_trail_m(&path)
    );
    assert!(
        min_dist_to_point(&geom, (500.0, 400.0)) < 60.0,
        "route should pass the divergent apex P(500,400)"
    );
}

#[test]
fn route_through_when_forced() {
    // Only ONE trail exists (A→Mid→B). Avoid the whole thing. The soft
    // (finite) penalty must still yield a route — never NoRoute — even
    // though both endpoints sit on the avoided trail.
    let scene = Scene::new(
        &[(0.0, 0.0), (500.0, 0.0), (1000.0, 0.0)],
        &[(0, 1), (1, 2)],
    );
    let mut prefs = base_prefs();
    prefs.avoid = vec![scene.avoid_line(&[(0.0, 0.0), (500.0, 0.0), (1000.0, 0.0)])];

    let path = scene
        .pf
        .solve(scene.ll(0.0, 0.0), scene.ll(1000.0, 0.0), prefs)
        .expect("soft avoidance must still return a route, not NoRoute");
    assert!(path.length_m > 900.0, "route should still connect A→B");
}

#[test]
fn no_shadow_walking() {
    // The key test. Graph = a direct trail A→B and a separate DIVERGENT
    // trail A→C→B; between them is only open (off-trail) terrain — the
    // tempting parallel gap. A round-trip-avoid route goes out on the
    // direct trail and must return on the DIVERGENT trail, with off-trail
    // metreage ~0. If the router "shadow-walks" parallel off-trail to
    // escape the avoided corridor instead, off_trail_m explodes and this
    // fails — which is the whole point.
    //
    // Nodes: 0=A(0,0) 1=Mid(500,0) 2=B(1000,0) 3=C(500,300)
    let scene = Scene::new(
        &[(0.0, 0.0), (500.0, 0.0), (1000.0, 0.0), (500.0, 300.0)],
        &[(0, 1), (1, 2), (0, 3), (3, 2)],
    );
    let mut prefs = base_prefs();
    prefs.round_trip = true;

    let loop_path = scene
        .pf
        .solve_route(&[scene.ll(0.0, 0.0), scene.ll(1000.0, 0.0)], prefs)
        .expect("round-trip loop should solve");
    let geom = scene.local_geom(&loop_path);

    // Whole loop is trail (out on direct, back on divergent): ~0 off-trail.
    // A shadow-walking return would put ~1000 m into off_trail.
    assert!(
        off_trail_m(&loop_path) < 40.0,
        "round-trip-avoid must NOT shadow-walk off-trail (off_trail {:.0} m)",
        off_trail_m(&loop_path)
    );
    // The return leg genuinely used the divergent trail: the loop passes
    // its apex C(500,300).
    assert!(
        min_dist_to_point(&geom, (500.0, 300.0)) < 60.0,
        "return leg should take the divergent trail via C(500,300)"
    );
    // Sanity: it is a real loop (out and back), so it's noticeably longer
    // than a one-way trip.
    assert!(
        loop_path.length_m > 1800.0,
        "loop should traverse out + back (len {:.0} m)",
        loop_path.length_m
    );
}

#[test]
fn graceful_loop_two_parallel_trails() {
    // Two parallel trails A↔B: the direct one (y=0) and a second bulging
    // 200 m north via D. Round-trip-avoid must go out on one and back on
    // the other — the return overlaps the outbound below threshold.
    //
    // Nodes: 0=A(0,0) 1=Mid(500,0) 2=B(1000,0) 3=D(500,200)
    let scene = Scene::new(
        &[(0.0, 0.0), (500.0, 0.0), (1000.0, 0.0), (500.0, 200.0)],
        &[(0, 1), (1, 2), (0, 3), (3, 2)],
    );
    let mut prefs = base_prefs();
    prefs.round_trip = true;

    let loop_path = scene
        .pf
        .solve_route(&[scene.ll(0.0, 0.0), scene.ll(1000.0, 0.0)], prefs)
        .expect("round-trip loop should solve");

    // Split the stitched loop into outbound + return legs.
    assert_eq!(loop_path.waypoint_legs.len(), 2);
    let geom = scene.local_geom(&loop_path);
    let ret = &loop_path.waypoint_legs[1];
    let return_geom: Vec<(f64, f64)> =
        geom[ret.geometry_start_idx as usize..=ret.geometry_end_idx as usize].to_vec();
    let out = &loop_path.waypoint_legs[0];
    let outbound_geom: Vec<(f64, f64)> =
        geom[out.geometry_start_idx as usize..=out.geometry_end_idx as usize].to_vec();

    let overlap = overlap_fraction(&return_geom, &outbound_geom, 30.0);
    assert!(
        overlap < 0.5,
        "return leg should use the OTHER trail (overlap with outbound {overlap:.2})"
    );
    assert!(
        off_trail_m(&loop_path) < 60.0,
        "both legs are trails (off_trail {:.0} m)",
        off_trail_m(&loop_path)
    );
}

#[test]
fn graceful_loop_single_trail_out_and_back() {
    // Only ONE trail (A→Mid→B). Round-trip-avoid can't diverge, so the
    // SOFT penalty must gracefully return an out-and-back rather than
    // failing with NoRoute.
    let scene = Scene::new(
        &[(0.0, 0.0), (500.0, 0.0), (1000.0, 0.0)],
        &[(0, 1), (1, 2)],
    );
    let mut prefs = base_prefs();
    prefs.round_trip = true;

    let loop_path = scene
        .pf
        .solve_route(&[scene.ll(0.0, 0.0), scene.ll(1000.0, 0.0)], prefs)
        .expect("single-trail round trip must still return a route (out-and-back)");
    // Out and back on the same corridor ⇒ roughly double the one-way length.
    assert!(
        loop_path.length_m > 1800.0,
        "out-and-back should traverse the trail twice (len {:.0} m)",
        loop_path.length_m
    );
}
