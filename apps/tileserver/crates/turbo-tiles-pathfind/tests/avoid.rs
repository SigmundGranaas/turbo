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

/// Single-tile DEM that is a UNIFORM planar ramp rising toward the south
/// (decreasing world-y) at `tan(alpha)` m rise per m of travel — `z(x,y) =
/// (uly - y) * tan(alpha)`, 10 m pixels. A steep face makes OFF-TRAIL travel
/// Tobler-expensive, which is the *real-terrain* regime the flat-DEM fixtures
/// can only fake by pinning a high `off_trail_base`. Copy of the switchback
/// fixture in `pathfinder.rs`. See the sloped tests at the bottom of the file.
fn write_ramp_dem(path: &std::path::Path, ulx: f64, uly: f64, cells: u32, alpha_deg: f64) {
    use turbo_tiles_artifacts::HEADER_BYTES;
    use turbo_tiles_elev::{
        write_meta as write_dem_meta, write_tile_entry, DemMeta, TileEntry, COMPRESSION_ZSTD,
        DEM_FORMAT_VERSION, DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
    };
    let pixel = 10.0_f64;
    let tan = alpha_deg.to_radians().tan();
    let meta = DemMeta {
        tile_count: 1,
        tile_cells: cells,
        pixel_size_m: pixel as f32,
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
    let mut data = vec![0f32; (cells * cells) as usize];
    for r in 0..cells {
        let z = (r as f64 * pixel * tan) as f32;
        for c in 0..cells {
            data[(r * cells + c) as usize] = z;
        }
    }
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

/// Ramp DEM (`alpha_deg` face) covering ~6 km around a test origin.
fn ramp_dem_around(
    ox: f64,
    oy: f64,
    alpha_deg: f64,
) -> (tempfile::NamedTempFile, Arc<turbo_tiles_elev::Dem>) {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_ramp_dem(tmp.path(), ox - 2000.0, oy + 2000.0, 600, alpha_deg);
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
    /// edges (node-index pairs, made bidirectional) over a FLAT DEM.
    fn new(local_nodes: &[(f64, f64)], edge_pairs: &[(u32, u32)]) -> Self {
        Self::build(local_nodes, edge_pairs, None)
    }

    /// Same, but over a UNIFORM `alpha_deg` planar ramp (real-terrain regime):
    /// off-trail travel is Tobler-expensive, so trail EDGES are the cheap line
    /// WITHOUT the artificial `off_trail_base` pin the flat fixtures need.
    fn new_sloped(local_nodes: &[(f64, f64)], edge_pairs: &[(u32, u32)], alpha_deg: f64) -> Self {
        Self::build(local_nodes, edge_pairs, Some(alpha_deg))
    }

    fn build(local_nodes: &[(f64, f64)], edge_pairs: &[(u32, u32)], slope_deg: Option<f64>) -> Self {
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
        let (dem_tmp, dem) = match slope_deg {
            Some(a) => ramp_dem_around(ox, oy, a),
            None => flat_dem_around(ox, oy),
        };
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

// =====================================================================
// Real-terrain regime (spec Phase 5 caveat)
//
// The flat-DEM tests above have to PIN `off_trail_base = 12.0` to make
// the edge-based avoid bite: on flat ground the `trail_proximity` bonus
// makes a mesh cell on a trail cheaper than the trail EDGE, so the
// solver rides trails as off-trail mesh and the edge penalty (graph
// edges only) never fires. The spec flags this: *does the avoid still
// bite on REAL terrain at the DEFAULT off_trail_base (2.3)?* These tests
// answer it — same scenarios on a uniform 35° face (steeper than the 27°
// soft-cap, gentler than the cliff veto), with `Prefs::default()` (NO
// off_trail_base pin). Slope makes off-trail Tobler-expensive, so trail
// edges become the cheap line on their own — the regime the model
// assumes — and the penalty bites without the crutch.
// =====================================================================

/// A real-terrain face used by every sloped test (steep enough that
/// off-trail is expensive, below the 45° mesh refuse / 50° edge refuse).
const REAL_FACE_DEG: f64 = 35.0;

#[test]
fn sloped_default_prefs_detours_onto_alternative_trail() {
    // Trails run N→S along the fall line (climbing the ramp). Direct trail
    // A→Mid→B at x=0; a divergent trail A→P→B bulges 400 m east. Avoiding
    // the direct corridor at the DEFAULT off_trail_base must route via the
    // real alternative trail (past P), NOT shadow-walk parallel off-trail
    // up the steep face.
    // Nodes: 0=A(0,0) 1=Mid(0,500) 2=B(0,1000) 3=P(400,500)
    let scene = Scene::new_sloped(
        &[(0.0, 0.0), (0.0, 500.0), (0.0, 1000.0), (400.0, 500.0)],
        &[(0, 1), (1, 2), (0, 3), (3, 2)],
        REAL_FACE_DEG,
    );
    let mut prefs = Prefs::default(); // <-- default off_trail_base (2.3), no pin
    prefs.avoid = vec![scene.avoid_line(&[(0.0, 0.0), (0.0, 500.0), (0.0, 1000.0)])];

    let path = scene
        .pf
        .solve(scene.ll(0.0, 0.0), scene.ll(0.0, 1000.0), prefs)
        .expect("a detour route exists on the sloped face");
    let geom = scene.local_geom(&path);

    // Un-avoided sanity: the default route hugs the direct corridor.
    let direct = scene
        .pf
        .solve(scene.ll(0.0, 0.0), scene.ll(0.0, 1000.0), Prefs::default())
        .unwrap();
    let direct_overlap =
        overlap_fraction(&scene.local_geom(&direct), &[(0.0, 0.0), (0.0, 1000.0)], 30.0);
    assert!(
        direct_overlap > 0.8,
        "sanity: un-avoided route hugs the direct corridor (overlap {direct_overlap:.2})"
    );

    let overlap = overlap_fraction(&geom, &[(0.0, 0.0), (0.0, 1000.0)], 30.0);
    assert!(
        overlap < 0.4,
        "avoided route should leave the direct corridor at default off_trail_base \
         (overlap {overlap:.2})"
    );
    // It escaped via the ALTERNATIVE TRAIL, not by shadow-walking the face:
    // off-trail metreage stays small even without the off_trail_base pin.
    assert!(
        off_trail_m(&path) < 120.0,
        "detour should ride the divergent trail, not the steep off-trail face \
         (off_trail {:.0} m)",
        off_trail_m(&path)
    );
    assert!(
        min_dist_to_point(&geom, (400.0, 500.0)) < 80.0,
        "route should pass the divergent apex P(400,500)"
    );
}

#[test]
fn sloped_default_prefs_no_shadow_walking() {
    // The key real-terrain test: round-trip self-avoidance at the DEFAULT
    // off_trail_base on a 35° face. Out on the direct trail, back on the
    // divergent trail (via C) — NOT a parallel off-trail shadow-walk up the
    // face. Nodes: 0=A(0,0) 1=Mid(0,500) 2=B(0,1000) 3=C(300,500)
    let scene = Scene::new_sloped(
        &[(0.0, 0.0), (0.0, 500.0), (0.0, 1000.0), (300.0, 500.0)],
        &[(0, 1), (1, 2), (0, 3), (3, 2)],
        REAL_FACE_DEG,
    );
    let mut prefs = Prefs::default(); // default off_trail_base — the real regime
    prefs.round_trip = true;

    let loop_path = scene
        .pf
        .solve_route(&[scene.ll(0.0, 0.0), scene.ll(0.0, 1000.0)], prefs)
        .expect("round-trip loop should solve on the sloped face");
    let geom = scene.local_geom(&loop_path);

    assert!(
        off_trail_m(&loop_path) < 120.0,
        "round-trip-avoid must NOT shadow-walk the face at default off_trail_base \
         (off_trail {:.0} m)",
        off_trail_m(&loop_path)
    );
    assert!(
        min_dist_to_point(&geom, (300.0, 500.0)) < 80.0,
        "return leg should take the divergent trail via C(300,500)"
    );
    assert!(
        loop_path.length_m > 1800.0,
        "loop should traverse out + back (len {:.0} m)",
        loop_path.length_m
    );
}

/// Characterization (run with `-- --nocapture`): quantifies the caveat by
/// printing off-trail metreage for the round-trip-avoid loop at the DEFAULT
/// off_trail_base on FLAT vs a 35° face. Flat is where the edge penalty is
/// blunted (more off-trail); the slope is what restores it. Not a brittle
/// pass/fail on the flat number — it's the evidence behind the two asserts
/// above — but it does assert the ordering (slope ≤ flat).
#[test]
fn sloped_vs_flat_off_trail_characterization() {
    let nodes = [(0.0, 0.0), (0.0, 500.0), (0.0, 1000.0), (300.0, 500.0)];
    let edges = [(0u32, 1u32), (1, 2), (0, 3), (3, 2)];

    let run = |scene: &Scene| -> f64 {
        let mut prefs = Prefs::default();
        prefs.round_trip = true;
        let loop_path = scene
            .pf
            .solve_route(&[scene.ll(0.0, 0.0), scene.ll(0.0, 1000.0)], prefs)
            .expect("loop solves");
        off_trail_m(&loop_path)
    };

    let flat = run(&Scene::new(&nodes, &edges));
    let sloped = run(&Scene::new_sloped(&nodes, &edges, REAL_FACE_DEG));
    eprintln!(
        "[phase5-eval] round-trip-avoid off_trail at DEFAULT off_trail_base: \
         flat={flat:.0}m  sloped(35°)={sloped:.0}m"
    );
    assert!(
        sloped <= flat + 1.0,
        "slope must not make shadow-walking WORSE than flat \
         (flat {flat:.0} m, sloped {sloped:.0} m)"
    );
}
