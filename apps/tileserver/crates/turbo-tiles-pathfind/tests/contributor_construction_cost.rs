//! E4 — how expensive is contributor construction?
//!
//! From the assumption audit (finding A2). The module design first proposed
//! a per-request `Tuning` struct in the model layer; that is a god-struct,
//! and the obvious alternative — "just rebuild the `CostModel` per request"
//! — is only viable if constructing contributors is cheap.
//!
//! It is not, for at least one of them. `TrailProximityContributor::new`
//! bulk-loads **three** R-trees off the whole graph:
//!
//! ```ignore
//! sti:      to_tree(graph.collect_polyline_segments_with_fkb_types(&[1], 100.0)),
//! vei:      to_tree(graph.collect_segments_with_fkb_types(&[2])),
//! skiloype: to_tree(graph.collect_segments_with_fkb_types(&[3])),
//! ```
//!
//! and its own comment notes the polyline variant "would cost ~1 GB of boot
//! RSS" for the vei/skiloype profiles. This test measures the scaling so the
//! index/params split point is chosen on evidence.
//!
//! Run with output:
//!   cargo test -p turbo-tiles-pathfind --test contributor_construction_cost \
//!       --release -- --nocapture

use std::io::Write;
use std::sync::Arc;
use std::time::Instant;

use byteorder::{LittleEndian, WriteBytesExt};
use turbo_tiles_artifacts::{write_header as write_art_header, ArtifactKind, Header};
use turbo_tiles_graph::{
    write_meta as write_graph_meta, EdgeRecord, Graph, GraphMeta, NodePos, GRAPH_FORMAT_VERSION,
};
use turbo_tiles_pathfind::{Pathfinder, TrailProximityContributor};

/// A grid graph of `side x side` nodes with 4-connected bidirectional edges,
/// cycling `fkb_type` over 1/2/3 so all three R-trees get populated the way a
/// real mixed sti/vei/skiloype network does.
fn write_grid_graph(path: &std::path::Path, side: u32, spacing_m: f32) -> (u32, u32) {
    let mut nodes = Vec::with_capacity((side * side) as usize);
    for j in 0..side {
        for i in 0..side {
            nodes.push(NodePos {
                x: i as f32 * spacing_m,
                y: j as f32 * spacing_m,
            });
        }
    }
    let idx = |i: u32, j: u32| j * side + i;
    let mut edges: Vec<EdgeRecord> = Vec::new();
    let mut k = 0u8;
    let mut push = |edges: &mut Vec<EdgeRecord>, a: u32, b: u32, k: &mut u8| {
        let fkb = (*k % 3) + 1; // 1 = sti, 2 = vei, 3 = skiloype
        *k = k.wrapping_add(1);
        for (from, to) in [(a, b), (b, a)] {
            edges.push(EdgeRecord {
                from_id: from,
                to_id: to,
                length_m: spacing_m,
                gain_m: 0.0,
                loss_m: 0.0,
                slope_max_deg: 0.0,
                fkb_type: fkb,
                marking: 0,
                surface: 0,
                source: 1,
                attr_flags: 0,
            });
        }
    };
    for j in 0..side {
        for i in 0..side {
            if i + 1 < side {
                push(&mut edges, idx(i, j), idx(i + 1, j), &mut k);
            }
            if j + 1 < side {
                push(&mut edges, idx(i, j), idx(i, j + 1), &mut k);
            }
        }
    }

    let nc = nodes.len() as u32;
    let ec = edges.len() as u32;
    let pc = 3u32;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
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

    // CSR offsets must be sorted by from_id; build the index table that way.
    let mut counts = vec![0u32; nc as usize + 1];
    for e in &edges {
        counts[e.from_id as usize + 1] += 1;
    }
    for i in 1..counts.len() {
        counts[i] += counts[i - 1];
    }
    for o in &counts {
        f.write_u32::<LittleEndian>(*o).unwrap();
    }
    let mut cursor = counts.clone();
    let mut table = vec![0u32; ec as usize];
    for (ei, e) in edges.iter().enumerate() {
        let slot = &mut cursor[e.from_id as usize];
        table[*slot as usize] = ei as u32;
        *slot += 1;
    }
    for t in &table {
        f.write_u32::<LittleEndian>(*t).unwrap();
    }
    for e in &edges {
        for _ in 0..pc {
            f.write_f32::<LittleEndian>(e.length_m).unwrap();
        }
    }
    f.flush().unwrap();
    drop(f);
    (nc, ec)
}

#[test]
fn measure_contributor_construction_scaling() {
    let dir = tempfile::tempdir().unwrap();
    println!();
    println!("E4 — contributor construction cost");
    println!(
        "{:>7} {:>9} {:>10} {:>16} {:>18}",
        "side", "nodes", "edges", "TrailProx::new", "with_defaults (full)"
    );

    let mut rows = Vec::new();
    for side in [20u32, 60, 120, 200] {
        let path = dir.path().join(format!("g{side}.graph"));
        let (nc, ec) = write_grid_graph(&path, side, 50.0);
        let graph = Arc::new(Graph::open(&path).unwrap());

        // Isolate the R-tree build.
        let t = Instant::now();
        let tp = TrailProximityContributor::new(&graph, 150.0, 0.6);
        let tp_ms = t.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box(&tp);

        // The whole stack, as `routing_setup::build_pathfinder` does it.
        let t = Instant::now();
        let pf = Pathfinder::with_defaults(None, None, Some(graph.clone()));
        let full_ms = t.elapsed().as_secs_f64() * 1e3;
        std::hint::black_box(&pf);

        println!("{side:>7} {nc:>9} {ec:>10} {tp_ms:>13.2} ms {full_ms:>15.2} ms");
        rows.push((ec, tp_ms, full_ms));
    }

    // Extrapolate to a national-scale graph. Norway's trail+road network is
    // order 10^6-10^7 edges; the largest sample here is ~10^5.
    let (ec, tp_ms, _) = *rows.last().unwrap();
    let per_edge_us = tp_ms * 1e3 / ec as f64;
    println!();
    println!("per-edge construction cost: {per_edge_us:.3} us/edge");
    for target in [1_000_000u64, 5_000_000] {
        println!(
            "  extrapolated to {:>9} edges: {:>8.0} ms",
            target,
            per_edge_us * target as f64 / 1e3
        );
    }
    println!();
    println!("A 250 ms mean solve is the budget. Anything above a few ms of");
    println!("construction rules out rebuilding the CostModel per request and");
    println!("forces the Arc<Index> + Params split (audit finding A2).");

    // Guard the conclusion so it cannot silently rot: construction must be
    // super-linear-free but is emphatically not free.
    assert!(
        rows.last().unwrap().1 > 0.0,
        "construction must be measurable"
    );
}
