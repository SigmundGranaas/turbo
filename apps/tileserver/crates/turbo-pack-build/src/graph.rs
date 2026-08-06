//! Write `norway.graph` and `norway.graph_geom` from line features.
//!
//! Same artifacts as `turbo-tiles-build::graph_builder`, and the same
//! cost function — both call `turbo_tiles_graph::profile_cost`. What
//! differs is the source: `paths.edge` there, N50 `Veglenke` and the FKB
//! sti WFS here, noded by [`crate::node`] instead of by
//! `pgr_createTopology`.
//!
//! # Directed pairs
//!
//! Every input line becomes two edges. The graph has no one-way concept,
//! and gain in one direction is loss in the other, so the pair is how
//! the artifact represents a walkable path rather than a modelling
//! choice to revisit.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use byteorder::{LittleEndian, WriteBytesExt};
use turbo_tiles_artifacts::{write_header, ArtifactKind, Header};
use turbo_tiles_elev::{Dem, PointXY};
use turbo_tiles_graph::{
    profile_cost, write_graph_geom_meta, write_meta, EdgeRecord, GraphGeomIndexEntry,
    GraphGeomMeta, GraphMeta, NodePos, GRAPH_FORMAT_VERSION, GRAPH_GEOM_FORMAT_VERSION,
    GRAPH_GEOM_INDEX_BYTES, PROFILE_COUNT,
};

use crate::node::{node_endpoints, TOLERANCE_M};
use crate::BuildError;
use crate::IoAt;

/// One input way: a polyline plus the classification the cost model reads.
#[derive(Debug, Clone)]
pub struct Way {
    /// EPSG:25833 vertices, in order.
    pub coords: Vec<(f64, f64)>,
    pub fkb_type: u8,
    pub marking: u8,
    pub surface: u8,
    /// Provenance byte, matching the server's `source` column.
    pub source: u8,
}

#[derive(Debug, Clone, Default)]
pub struct GraphReport {
    pub nodes: u32,
    pub edges_directed: u32,
    pub ways_in: usize,
    pub ways_dropped_degenerate: usize,
    pub edges_without_terrain: u32,
    pub graph_bytes: u64,
    pub geom_bytes: u64,
}

/// Build both graph artifacts into `out_dir`.
///
/// `dem` is optional and its absence is honest rather than fatal: with
/// no terrain, `gain_m` and `slope_max_deg` stay zero and the cost falls
/// back to distance. That is the same thing the server builder does when
/// the DEM does not cover an edge, and it is better than guessing a
/// climb that would move routes.
pub fn build(
    ways: &[Way],
    dem: Option<&Dem>,
    out_dir: &Path,
) -> Result<(PathBuf, PathBuf, GraphReport), BuildError> {
    std::fs::create_dir_all(out_dir).at(out_dir)?;
    let mut report = GraphReport {
        ways_in: ways.len(),
        ..Default::default()
    };

    // Keep only ways that can carry a direction. A single-vertex or
    // zero-length way would produce an edge whose endpoints node
    // together, i.e. a self-loop the solver can sit on for free.
    let usable: Vec<&Way> = ways
        .iter()
        .filter(|w| {
            if w.coords.len() < 2 {
                return false;
            }
            let (a, b) = (w.coords[0], w.coords[w.coords.len() - 1]);
            let span = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            // A closed loop is legitimate (a circular trail) as long as
            // it has real length along the way.
            span > TOLERANCE_M || polyline_length(&w.coords) > TOLERANCE_M
        })
        .collect();
    report.ways_dropped_degenerate = ways.len() - usable.len();

    let ends: Vec<(f64, f64, f64, f64)> = usable
        .iter()
        .map(|w| {
            let a = w.coords[0];
            let b = w.coords[w.coords.len() - 1];
            (a.0, a.1, b.0, b.1)
        })
        .collect();
    let topo = node_endpoints(&ends, TOLERANCE_M);

    let nodes: Vec<NodePos> = topo
        .nodes
        .iter()
        .map(|&(x, y)| NodePos {
            x: x as f32,
            y: y as f32,
        })
        .collect();

    let mut edges: Vec<EdgeRecord> = Vec::with_capacity(usable.len() * 2);
    let mut polylines: Vec<Vec<NodePos>> = Vec::with_capacity(usable.len() * 2);

    for (i, w) in usable.iter().enumerate() {
        let u = topo.node_of[i * 2];
        let v = topo.node_of[i * 2 + 1];

        let mut fwd: Vec<NodePos> = w
            .coords
            .iter()
            .map(|&(x, y)| NodePos {
                x: x as f32,
                y: y as f32,
            })
            .collect();
        // Snap the polyline ends onto the node positions, which moved to
        // the centroid of what merged. Leaving them apart would make the
        // reconstructed route jump by up to a tolerance at every
        // junction.
        if let Some(f) = fwd.first_mut() {
            *f = nodes[u as usize];
        }
        if let Some(l) = fwd.last_mut() {
            *l = nodes[v as usize];
        }
        let mut rev = fwd.clone();
        rev.reverse();

        let length = polyline_length(&w.coords) as f32;
        let (gain, loss, slope) = terrain(&fwd, dem);
        if dem.is_some() && gain == 0.0 && loss == 0.0 && slope == 0.0 {
            report.edges_without_terrain += 1;
        }

        edges.push(EdgeRecord {
            from_id: u,
            to_id: v,
            length_m: length,
            gain_m: gain,
            loss_m: loss,
            slope_max_deg: slope,
            fkb_type: w.fkb_type,
            marking: w.marking,
            surface: w.surface,
            source: w.source,
            attr_flags: 0,
        });
        polylines.push(fwd);

        // Reverse: gain and loss swap; max slope is symmetric.
        edges.push(EdgeRecord {
            from_id: v,
            to_id: u,
            length_m: length,
            gain_m: loss,
            loss_m: gain,
            slope_max_deg: slope,
            fkb_type: w.fkb_type,
            marking: w.marking,
            surface: w.surface,
            source: w.source,
            attr_flags: 0,
        });
        polylines.push(rev);
    }

    // Sort by from_id so CSR offsets are O(E). Permute both arrays
    // together — `polylines` is parallel to `edges`, and sorting one
    // alone would give every edge a different edge's geometry, which
    // reconstructs routes out of unrelated fragments.
    let mut perm: Vec<u32> = (0..edges.len() as u32).collect();
    perm.sort_by_key(|&i| edges[i as usize].from_id);
    let edges: Vec<EdgeRecord> = perm.iter().map(|&i| edges[i as usize]).collect();
    let polylines: Vec<Vec<NodePos>> = perm
        .iter()
        .map(|&i| std::mem::take(&mut polylines[i as usize]))
        .collect();

    let nc = nodes.len() as u32;
    let ec = edges.len() as u32;
    report.nodes = nc;
    report.edges_directed = ec;

    let mut offsets: Vec<u32> = vec![0; nc as usize + 1];
    for e in &edges {
        offsets[e.from_id as usize + 1] += 1;
    }
    for i in 1..offsets.len() {
        offsets[i] += offsets[i - 1];
    }

    // ---- norway.graph ----
    let out_path = out_dir.join(ArtifactKind::Graph.filename());
    let tmp = out_dir.join(format!("{}.tmp", ArtifactKind::Graph.filename()));
    {
        let mut w = BufWriter::with_capacity(4 << 20, File::create(&tmp).at(&tmp)?);
        write_header(
            &mut w,
            &Header {
                kind: ArtifactKind::Graph,
                format_version: GRAPH_FORMAT_VERSION,
                build_timestamp_unix_sec: chrono::Utc::now().timestamp(),
            },
        )
        .map_err(|e| BuildError::Logic(format!("graph header: {e}")))?;
        write_meta(
            &mut w,
            &GraphMeta {
                node_count: nc,
                edge_count: ec,
                profile_count: PROFILE_COUNT,
                srid: 25833,
            },
        )?;
        w.write_all(bytemuck::cast_slice(&nodes))?;
        w.write_all(bytemuck::cast_slice(&edges))?;
        for o in &offsets {
            w.write_u32::<LittleEndian>(*o)?;
        }
        for e in 0..ec {
            w.write_u32::<LittleEndian>(e)?;
        }
        for e in &edges {
            for p in 0..PROFILE_COUNT {
                w.write_f32::<LittleEndian>(profile_cost(e, p))?;
            }
        }
        w.flush()?;
    }
    std::fs::rename(&tmp, &out_path).at(&tmp)?;
    report.graph_bytes = std::fs::metadata(&out_path).at(&out_path)?.len();

    // ---- norway.graph_geom ----
    let geom_path = out_dir.join(ArtifactKind::GraphGeom.filename());
    let geom_tmp = out_dir.join(format!("{}.tmp", ArtifactKind::GraphGeom.filename()));
    {
        let mut w = BufWriter::with_capacity(4 << 20, File::create(&geom_tmp).at(&geom_tmp)?);
        let total: u32 = polylines.iter().map(|p| p.len() as u32).sum();
        write_header(
            &mut w,
            &Header {
                kind: ArtifactKind::GraphGeom,
                format_version: GRAPH_GEOM_FORMAT_VERSION,
                build_timestamp_unix_sec: chrono::Utc::now().timestamp(),
            },
        )
        .map_err(|e| BuildError::Logic(format!("graph_geom header: {e}")))?;
        write_graph_geom_meta(
            &mut w,
            &GraphGeomMeta {
                edge_count: ec,
                total_vertices: total,
            },
        )?;
        let mut index = Vec::with_capacity(ec as usize * GRAPH_GEOM_INDEX_BYTES);
        let mut acc: u32 = 0;
        for p in &polylines {
            index.extend_from_slice(bytemuck::bytes_of(&GraphGeomIndexEntry {
                offset: acc,
                count: p.len() as u32,
            }));
            acc = acc.saturating_add(p.len() as u32);
        }
        w.write_all(&index)?;
        for p in &polylines {
            w.write_all(bytemuck::cast_slice(p))?;
        }
        w.flush()?;
    }
    std::fs::rename(&geom_tmp, &geom_path).at(&geom_tmp)?;
    report.geom_bytes = std::fs::metadata(&geom_path).at(&geom_path)?.len();

    Ok((out_path, geom_path, report))
}

fn polyline_length(coords: &[(f64, f64)]) -> f64 {
    coords
        .windows(2)
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .sum()
}

/// Gain, loss and max slope along a polyline, sampled from the DEM.
///
/// All-or-nothing: if any vertex falls outside the DEM the whole edge
/// reports zero rather than a partial climb. A partial figure would be
/// an understatement that looks like flat ground, and flat ground is
/// exactly what a router prefers — so the edge would attract routes
/// *because* its terrain is missing.
fn terrain(points: &[NodePos], dem: Option<&Dem>) -> (f32, f32, f32) {
    let Some(d) = dem else {
        return (0.0, 0.0, 0.0);
    };
    let mut elevations = Vec::with_capacity(points.len());
    for p in points {
        match d
            .sample(PointXY {
                x: p.x as f64,
                y: p.y as f64,
            })
            .ok()
            .flatten()
        {
            Some(v) => elevations.push(v),
            None => return (0.0, 0.0, 0.0),
        }
    }
    if elevations.len() < 2 {
        return (0.0, 0.0, 0.0);
    }
    let (mut gain, mut loss, mut slope_max) = (0.0f32, 0.0f32, 0.0f32);
    for (seg, els) in points.windows(2).zip(elevations.windows(2)) {
        let dx = (seg[1].x - seg[0].x) as f64;
        let dy = (seg[1].y - seg[0].y) as f64;
        let horiz = (dx * dx + dy * dy).sqrt();
        // Below a metre the slope is noise: a 0.3 m step with a 1 m
        // elevation difference is a 73° cliff that is really just DEM
        // quantisation.
        if horiz < 1.0 {
            continue;
        }
        let dz = (els[1] - els[0]) as f64;
        let deg = (dz / horiz).atan().to_degrees().abs() as f32;
        if deg > slope_max {
            slope_max = deg;
        }
        if dz > 0.0 {
            gain += dz as f32;
        } else {
            loss += (-dz) as f32;
        }
    }
    (gain, loss, slope_max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use turbo_tiles_graph::Graph;

    fn way(coords: Vec<(f64, f64)>, fkb_type: u8) -> Way {
        Way {
            coords,
            fkb_type,
            marking: 0,
            surface: 0,
            source: 1,
        }
    }

    /// The round trip: what this writes is what the engine's own reader
    /// opens, with the topology the noder found.
    #[test]
    fn a_built_graph_opens_through_the_real_reader() {
        let dir = tempfile::tempdir().unwrap();
        let ways = vec![
            way(vec![(0.0, 0.0), (100.0, 0.0)], 1),
            way(vec![(100.0, 0.0), (200.0, 50.0)], 1),
        ];
        let (path, geom, report) = build(&ways, None, dir.path()).unwrap();

        assert_eq!(report.nodes, 3, "the shared endpoint should merge");
        assert_eq!(report.edges_directed, 4, "each way is a directed pair");

        let mut g = Graph::open(&path).expect("the graph crate must open what we wrote");
        assert_eq!(g.meta().node_count, 3);
        assert_eq!(g.meta().edge_count, 4);
        assert_eq!(g.meta().srid, 25833);
        assert!(g.attach_geom(&geom).expect("attach geom"));
        assert!(g.has_geom());
    }

    /// CSR indexes edges by a run per node, so the edge array MUST be
    /// sorted by `from_id`. Unsorted, every offset past the first break
    /// points at another node's edges — a silent misroute, not a crash.
    #[test]
    fn edges_are_grouped_by_from_id_as_csr_requires() {
        let dir = tempfile::tempdir().unwrap();
        let ways = vec![
            way(vec![(0.0, 0.0), (100.0, 0.0)], 1),
            way(vec![(100.0, 0.0), (200.0, 0.0)], 1),
            way(vec![(100.0, 0.0), (100.0, 100.0)], 1),
        ];
        let (path, _, _) = build(&ways, None, dir.path()).unwrap();
        let g = Graph::open(&path).unwrap();
        assert_eq!(g.meta().edge_count, 6);

        let mut last = 0u32;
        let mut degree = std::collections::BTreeMap::new();
        for i in 0..g.meta().edge_count {
            let e = g.edge(i).expect("edge in range");
            assert!(e.from_id >= last, "edge {i} breaks from_id ordering");
            last = e.from_id;
            *degree.entry(e.from_id).or_insert(0usize) += 1;
        }
        // The junction has three ways meeting, so three outgoing edges.
        assert!(
            degree.values().any(|&d| d == 3),
            "no node has 3 outgoing edges: {degree:?}"
        );
    }

    #[test]
    fn geometry_stays_paired_with_its_edge_after_the_csr_sort() {
        let dir = tempfile::tempdir().unwrap();
        // Distinct bends, so a swapped polyline is detectable.
        let ways = vec![
            way(vec![(0.0, 0.0), (50.0, 40.0), (100.0, 0.0)], 1),
            way(vec![(100.0, 0.0), (150.0, -70.0), (200.0, 0.0)], 1),
        ];
        let (path, geom, _) = build(&ways, None, dir.path()).unwrap();
        let mut g = Graph::open(&path).unwrap();
        g.attach_geom(&geom).unwrap();
        for e in 0..g.meta().edge_count {
            let line = g.edge_polyline(e);
            assert_eq!(line.len(), 3, "edge {e} lost a vertex");
            let rec_from = g.node(g.edge(e).unwrap().from_id).unwrap();
            assert!(
                (line[0].x - rec_from.x).abs() < 0.01 && (line[0].y - rec_from.y).abs() < 0.01,
                "edge {e}: polyline starts at {:?}, node is {rec_from:?}",
                line[0]
            );
        }
    }

    #[test]
    fn a_degenerate_way_is_dropped_rather_than_becoming_a_self_loop() {
        let dir = tempfile::tempdir().unwrap();
        let ways = vec![
            way(vec![(0.0, 0.0), (100.0, 0.0)], 1),
            way(vec![(500.0, 500.0)], 1),
            way(vec![(600.0, 600.0), (600.2, 600.1)], 1),
        ];
        let (path, _, report) = build(&ways, None, dir.path()).unwrap();
        assert_eq!(report.ways_dropped_degenerate, 2);
        let g = Graph::open(&path).unwrap();
        assert_eq!(g.meta().edge_count, 2);
    }

    #[test]
    fn with_no_dem_the_cost_is_distance_only() {
        let dir = tempfile::tempdir().unwrap();
        let ways = vec![way(vec![(0.0, 0.0), (1000.0, 0.0)], 1)];
        let (path, _, _) = build(&ways, None, dir.path()).unwrap();
        let g = Graph::open(&path).unwrap();
        // Foot profile on a flat trail: cost == length.
        assert!(
            (g.edge_cost(0, 0) - 1000.0).abs() < 0.5,
            "{}",
            g.edge_cost(0, 0)
        );
    }
}
