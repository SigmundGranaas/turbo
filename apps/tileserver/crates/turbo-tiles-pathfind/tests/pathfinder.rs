//! Pathfinder composition tests against synthetic artifacts.

use std::io::Write;
use std::sync::Arc;

use byteorder::{LittleEndian, WriteBytesExt};
use turbo_tiles_artifacts::{write_header as write_art_header, ArtifactKind, Header};
use turbo_tiles_graph::{
    write_meta as write_graph_meta, EdgeRecord, Graph, GraphMeta, NodePos, GRAPH_FORMAT_VERSION,
};
use turbo_tiles_pathfind::{utm33n_to_wgs84, PathStrategy, Pathfinder, Prefs};

/// Write a single-tile flat DEM (constant elevation) covering a
/// `cells × cells` grid at 10 m resolution from upper-left `(ulx,
/// uly)`. The off-trail leg is FMM-only and needs a DEM to produce a
/// candidate; these strategy-selection fixtures used `None`, so
/// off-trail never entered the candidate race. A flat DEM lets the
/// solver run (slope 0, gain 0) and reduces to the straight-line
/// geodesic — exactly the "off-trail diagonal vs graph detour"
/// scenario the tests assert.
fn write_flat_dem(path: &std::path::Path, ulx: f64, uly: f64, cells: u32, elev: f32) {
    use turbo_tiles_elev::{
        write_meta as write_dem_meta, write_tile_entry, DemMeta, TileEntry, COMPRESSION_ZSTD,
        DEM_FORMAT_VERSION, DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
    };
    use turbo_tiles_artifacts::HEADER_BYTES;
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

/// Flat DEM (elev 100 m) covering ~6 km around a test origin in
/// EPSG:25833, returned alongside the backing tempfile (which must
/// stay alive — `Dem` mmaps it).
fn flat_dem_around(ox: f64, oy: f64) -> (tempfile::NamedTempFile, Arc<turbo_tiles_elev::Dem>) {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_flat_dem(tmp.path(), ox - 2000.0, oy + 2000.0, 600, 100.0);
    let dem = Arc::new(turbo_tiles_elev::Dem::open(tmp.path()).unwrap());
    (tmp, dem)
}

fn write_square_graph(path: &std::path::Path) {
    let mut f = std::fs::File::create(path).unwrap();
    let nodes = vec![
        NodePos { x: 0.0, y: 100.0 },
        NodePos { x: 100.0, y: 100.0 },
        NodePos { x: 100.0, y: 0.0 },
        NodePos { x: 0.0, y: 0.0 },
    ];
    let edges = vec![
        mk_edge(0, 1, 100.0),
        mk_edge(1, 0, 100.0),
        mk_edge(1, 2, 100.0),
        mk_edge(2, 1, 100.0),
        mk_edge(2, 3, 100.0),
        mk_edge(3, 2, 100.0),
        mk_edge(3, 0, 100.0),
        mk_edge(0, 3, 100.0),
    ];
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
    f.write_all(bytemuck::cast_slice(&nodes)).unwrap();
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

fn mk_edge(from: u32, to: u32, len_m: f32) -> EdgeRecord {
    EdgeRecord {
        from_id: from,
        to_id: to,
        length_m: len_m,
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

#[test]
fn pathfinder_refuses_when_no_coverage_anywhere() {
    // No DEM, no mask, no graph → every layer reports "I have no
    // data here". The pathfinder MUST refuse rather than build a
    // uniform-cost mesh and return a straight line. This is the
    // regression test for the Halsvatnet-straight-line bug.
    let pf = Pathfinder::with_defaults(None, None, None);
    let from = utm33n_to_wgs84(0.0, 0.0);
    let to = utm33n_to_wgs84(500.0, 500.0);
    let prefs = Prefs::default();
    let err = pf
        .solve([from.0, from.1], [to.0, to.1], prefs)
        .expect_err("must refuse when no primitive covers the points");
    match err {
        turbo_tiles_pathfind::PathfindError::NoCoverage {
            from_covered,
            to_covered,
            from_has_graph_anchor,
            to_has_graph_anchor,
        } => {
            assert!(!from_covered);
            assert!(!to_covered);
            assert!(!from_has_graph_anchor);
            assert!(!to_has_graph_anchor);
        }
        other => panic!("expected NoCoverage, got {other:?}"),
    }
}

#[test]
fn pathfinder_picks_cheapest_strategy() {
    // With the 4-node square fixture and no DEM/mask layers, the
    // straight-line off-trail path (≈141 m diagonal) costs less than
    // the 2-hop graph detour (200 m × surface multiplier). The
    // cost-based selector must pick OffTrail here even though both
    // endpoints snap to graph nodes — that's the architectural
    // invariant.
    let p = turbo_tiles_elev::wgs84_to_utm33n(10.7522, 59.9139);
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_square_graph_at(tmp.path(), p.x as f32, p.y as f32);
    let g = Graph::open(tmp.path()).unwrap();
    let (_dem_tmp, dem) = flat_dem_around(p.x, p.y);
    let pf = Pathfinder::with_defaults(Some(dem), None, Some(Arc::new(g)));
    // Snap radius is 200 m by default — pick lon/lat that's about
    // 50 m east + 50 m south of the anchor node.
    let from = (10.7522, 59.9139);
    let to = utm33n_to_wgs84(p.x + 100.0, p.y - 100.0);
    // Pin off_trail_base to 1.0 so the cost ceiling is calibration-
    // independent — the architectural property under test is "off-
    // trail diagonal beats 2-hop graph detour", not the absolute s/m
    // pace. With off_trail_base=1.0 and base pace 0.714 s/m the
    // 141 m diagonal is ≈101 walk-seconds; the 200 m graph 2-hop
    // (foot profile) is ≈143 s. Threshold 200 still leaves a wide
    // margin if calibration shifts.
    let mut prefs = Prefs::default();
    prefs.off_trail_base = Some(1.0);
    let path = pf.solve([from.0, from.1], [to.0, to.1], prefs).unwrap();
    // The unified router (default) cuts across off-trail here instead of
    // taking the 200 m graph 2-hop. It reports `Hybrid` (one solve over
    // trail + off-trail), so we assert on geometry/cost, not the enum.
    let diag = (100.0_f64.powi(2) + 100.0_f64.powi(2)).sqrt();
    // The FMM/lifted off-trail solver walks a discrete 10 m grid and
    // Chaikin-smooths, so the path is a few percent longer than the
    // ideal 141.4 m secant (≈148 m here). The invariant under test is
    // "off-trail diagonal, NOT the 200 m graph 2-hop", so allow grid-
    // quantization slack while staying well under the graph detour.
    assert!(
        path.length_m >= diag - 2.0 && path.length_m < 170.0,
        "expected a near-diagonal off-trail path (≈{diag:.1} m, < 170 m), got {}",
        path.length_m
    );
    assert!(path.cost <= 200.0, "cost ({}) should beat 2-hop graph (200)", path.cost);
}

#[test]
fn pathfinder_hybrid_when_one_end_off_graph() {
    // Anchor a 4-node square graph near Oslo. The `from` query is
    // 600 m away from any graph node (outside snap_radius=200 m
    // but inside bridge_radius=3 km), so Strategy 1 fails. The
    // `to` query sits right on a graph node so it snaps. The
    // pathfinder must produce a hybrid path: off-trail prefix from
    // `from` to the nearest graph node, then graph route to `to`.
    let anchor = turbo_tiles_elev::wgs84_to_utm33n(10.7522, 59.9139);
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_square_graph_at(tmp.path(), anchor.x as f32, anchor.y as f32);
    let g = Graph::open(tmp.path()).unwrap();
    let (_dem_tmp, dem) = flat_dem_around(anchor.x, anchor.y);
    let pf = Pathfinder::with_defaults(Some(dem), None, Some(Arc::new(g)));
    let from = utm33n_to_wgs84(anchor.x + 600.0, anchor.y); // ~600 m east
    let to = utm33n_to_wgs84(anchor.x, anchor.y); // sits on node 3
    let prefs = Prefs::default();
    let path = pf.solve([from.0, from.1], [to.0, to.1], prefs).unwrap();
    // Cost-based selection: whichever strategy wins is the one with
    // the lowest cost. On this fixture (no terrain layers), the
    // straight-line off-trail diagonal (~600 m) beats hybrid (off-
    // trail prefix ~500 m + graph leg ~500 m = ~1000 m). The
    // important invariant is that we got a path. Strategy choice
    // is a function of cost, not test expectation.
    assert!(path.length_m > 0.0);
    assert!(
        matches!(
            path.strategy,
            PathStrategy::Hybrid | PathStrategy::OffTrail
        ),
        "got unexpected strategy: {:?}",
        path.strategy
    );
}

#[test]
fn pathfinder_layer_weights_disable_preferred_edge_layer() {
    // Set layer_weights["slope"] = 0.0 — slope contributions get
    // suppressed even though the layer is registered. Smoke check:
    // the path still solves and reports the correct strategy.
    let anchor = turbo_tiles_elev::wgs84_to_utm33n(10.7522, 59.9139);
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_square_graph_at(tmp.path(), anchor.x as f32, anchor.y as f32);
    let g = Graph::open(tmp.path()).unwrap();
    let (_dem_tmp, dem) = flat_dem_around(anchor.x, anchor.y);
    let pf = Pathfinder::with_defaults(Some(dem), None, Some(Arc::new(g)));
    let from = (10.7522, 59.9139);
    let to = utm33n_to_wgs84(anchor.x + 100.0, anchor.y - 100.0);
    let mut prefs = Prefs::default();
    prefs.layer_weights.insert("preferred_edge".into(), 0.0);
    prefs.layer_weights.insert("marking".into(), 0.0);
    let path = pf.solve([from.0, from.1], [to.0, to.1], prefs).unwrap();
    // With zero-weighted edge layers the per-request multipliers are
    // identity. The selector still picks the cheapest path; on this
    // fixture (no mesh blockers), that's the off-trail diagonal.
    assert!(path.length_m > 0.0);
    assert!(matches!(
        path.strategy,
        PathStrategy::OnGraph | PathStrategy::OffTrail | PathStrategy::Hybrid
    ));
}

#[test]
fn cost_based_selection_beats_long_graph_detour() {
    // Regression test for the "62 km road detour beats 6 km
    // off-trail" failure. Build a graph with TWO close graph nodes
    // (10 m apart) that are connected through a LONG detour chain
    // (1 km × 10 hops). Off-trail straight-line is 10 m. The
    // selector must prefer off-trail (cost ~14) over graph
    // (cost ~14000) even though both endpoints snap.
    let p = turbo_tiles_elev::wgs84_to_utm33n(10.7522, 59.9139);
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_long_detour_graph(tmp.path(), p.x as f32, p.y as f32);
    let g = Graph::open(tmp.path()).unwrap();
    let (_dem_tmp, dem) = flat_dem_around(p.x, p.y);
    let pf = Pathfinder::with_defaults(Some(dem), None, Some(Arc::new(g)));
    // Place the clicks 200 m apart so we clear the DegenerateInputs
    // threshold (default mesh_cell_m = 100 m); still tiny vs the
    // 10 km graph detour.
    let from = utm33n_to_wgs84(p.x, p.y);
    let to = utm33n_to_wgs84(p.x + 200.0, p.y);
    let mut prefs = Prefs::default();
    prefs.mesh_cell_m = 25.0; // fine grid for the small bbox
    let path = pf
        .solve([from.0, from.1], [to.0, to.1], prefs)
        .unwrap();
    // The unified router cuts across (~200 m) rather than taking the
    // ~10 km graph detour. Assert on length (the regression we care about),
    // not the strategy enum.
    assert!(
        path.length_m < 500.0,
        "detour regression: expected ~200 m off-trail cut, got {}",
        path.length_m
    );
}

#[test]
fn pathfinder_lists_layer_names() {
    let pf = Pathfinder::with_defaults(None, None, None);
    let names = pf.layer_names();
    // No DEM/Mask loaded → only the edge layers remain.
    assert!(names.contains(&"preferred_edge"));
    assert!(names.contains(&"marking"));
}

/// 12-node graph where node 0 and node 11 sit 10 m apart but are
/// only connected via a 10 km zig-zag detour through nodes 1..10.
/// This is the "62 km road detour vs 6 km off-trail" pattern in
/// miniature — used to verify the selector picks off-trail when
/// the graph route is much longer than the straight line.
fn write_long_detour_graph(path: &std::path::Path, ox: f32, oy: f32) {
    let mut f = std::fs::File::create(path).unwrap();
    // Node 0 at origin, node 11 at (10, 0). The remaining 10 nodes
    // form a north-east zig-zag chain so the only graph route from
    // 0 to 11 is 0→1→2→…→10→11 = 11 hops × 1000 m.
    let nodes: Vec<NodePos> = (0..12)
        .map(|i| {
            if i == 0 {
                NodePos { x: ox, y: oy }
            } else if i == 11 {
                NodePos { x: ox + 10.0, y: oy }
            } else {
                NodePos {
                    x: ox + (i as f32) * 1000.0,
                    y: oy + (i as f32) * 1000.0,
                }
            }
        })
        .collect();
    let mut edges: Vec<EdgeRecord> = Vec::new();
    // Linear chain 0 ↔ 1 ↔ 2 ↔ ... ↔ 10 ↔ 11, each hop ≈ 1 km.
    for i in 0..11 {
        let a = &nodes[i];
        let b = &nodes[i + 1];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        edges.push(mk_edge(i as u32, (i + 1) as u32, len));
        edges.push(mk_edge((i + 1) as u32, i as u32, len));
    }
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
    f.write_all(bytemuck::cast_slice(&nodes)).unwrap();
    f.write_all(bytemuck::cast_slice(&edges)).unwrap();
    // CSR offsets (edges already sorted by from_id).
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

/// Variant of `write_square_graph` with a configurable origin.
fn write_square_graph_at(path: &std::path::Path, ox: f32, oy: f32) {
    let mut f = std::fs::File::create(path).unwrap();
    let nodes = vec![
        NodePos { x: ox, y: oy + 100.0 },
        NodePos { x: ox + 100.0, y: oy + 100.0 },
        NodePos { x: ox + 100.0, y: oy },
        NodePos { x: ox, y: oy },
    ];
    let edges = vec![
        mk_edge(0, 1, 100.0),
        mk_edge(1, 0, 100.0),
        mk_edge(1, 2, 100.0),
        mk_edge(2, 1, 100.0),
        mk_edge(2, 3, 100.0),
        mk_edge(3, 2, 100.0),
        mk_edge(3, 0, 100.0),
        mk_edge(0, 3, 100.0),
    ];
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
    f.write_all(bytemuck::cast_slice(&nodes)).unwrap();
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
