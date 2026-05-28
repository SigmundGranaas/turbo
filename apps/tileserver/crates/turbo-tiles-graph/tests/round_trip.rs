//! End-to-end round trip for the graph artifact format + Dijkstra.

use std::io::Write;

use byteorder::{LittleEndian, WriteBytesExt};
use turbo_tiles_artifacts::{write_header, ArtifactKind, Header};
use turbo_tiles_graph::{
    write_meta, EdgeRecord, Graph, GraphMeta, NodePos, Profile, GRAPH_FORMAT_VERSION,
};

/// Layout:
///   0 ── 1
///   │    │
///   3 ── 2
/// Each side = 100 m. Square graph; all four nodes reachable.
fn write_square_artifact(path: &std::path::Path, cost_multiplier: f32) {
    let mut f = std::fs::File::create(path).unwrap();
    let nodes = vec![
        NodePos { x: 0.0, y: 100.0 },   // 0
        NodePos { x: 100.0, y: 100.0 }, // 1
        NodePos { x: 100.0, y: 0.0 },   // 2
        NodePos { x: 0.0, y: 0.0 },     // 3
    ];
    let edges = vec![
        // bidirectional: each undirected edge has two records.
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
    let meta = GraphMeta {
        node_count: nc,
        edge_count: ec,
        profile_count: pc,
        srid: 25833,
    };
    write_header(
        &mut f,
        &Header {
            kind: ArtifactKind::Graph,
            format_version: GRAPH_FORMAT_VERSION,
            build_timestamp_unix_sec: 1_700_000_000,
        },
    )
    .unwrap();
    write_meta(&mut f, &meta).unwrap();
    let nb: &[u8] = bytemuck::cast_slice(&nodes);
    f.write_all(nb).unwrap();
    let eb: &[u8] = bytemuck::cast_slice(&edges);
    f.write_all(eb).unwrap();
    // CSR offsets — sort edges by from_id (test fixture already sorted).
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
    // CSR edge index list — in our fixture the index equals the edge
    // position because we constructed them in from-id order.
    for i in 0..ec {
        f.write_u32::<LittleEndian>(i).unwrap();
    }
    // Per-profile costs: cost = length_m × cost_multiplier for every profile.
    for e in &edges {
        for _ in 0..pc {
            let c = e.length_m * cost_multiplier;
            f.write_f32::<LittleEndian>(c).unwrap();
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
fn route_picks_two_step_diagonal() {
    // 0 to 2 should go 0 → 1 → 2 (or 0 → 3 → 2). Cost = 200.
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_square_artifact(tmp.path(), 1.0);
    let g = Graph::open(tmp.path()).unwrap();
    let r = g.route(0, 2, Profile::Foot).unwrap().expect("route");
    assert_eq!(r.length_m, 200.0);
    assert_eq!(r.edges.len(), 2);
    assert_eq!(r.nodes.len(), 3);
    assert_eq!(r.nodes[0], 0);
    assert_eq!(*r.nodes.last().unwrap(), 2);
}

#[test]
fn snap_finds_nearest_node() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_square_artifact(tmp.path(), 1.0);
    let g = Graph::open(tmp.path()).unwrap();
    // Near node 1 (100,100).
    let id = g.snap(95.0, 99.0, 50.0).unwrap();
    assert_eq!(id, 1);
}

#[test]
fn snap_rejects_outside_radius() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_square_artifact(tmp.path(), 1.0);
    let g = Graph::open(tmp.path()).unwrap();
    let err = g.snap(1000.0, 1000.0, 10.0).unwrap_err();
    assert!(err.to_string().contains("snap failed"));
}

#[test]
fn stats_reports_extent() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    write_square_artifact(tmp.path(), 1.0);
    let g = Graph::open(tmp.path()).unwrap();
    let s = g.stats();
    assert_eq!(s.meta.node_count, 4);
    assert_eq!(s.meta.edge_count, 8);
    assert!((s.min_x - 0.0).abs() < 1e-3);
    assert!((s.max_x - 100.0).abs() < 1e-3);
}
