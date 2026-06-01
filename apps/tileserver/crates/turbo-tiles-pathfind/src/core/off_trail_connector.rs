//! Pure off-trail connector. Given a built local mesh with a start
//! node and at least one exit, find the cheapest Theta\* path from
//! start to any exit.
//!
//! This is the bridge that turns a user-clicked lon/lat into a
//! sequence of (off-trail segments) + (target graph node). The graph
//! takes over from there.

use super::off_trail::{theta_star, MeshNodeId, PathResult, Point2};
use super::off_trail_mesh::BuiltMesh;

/// Result of finding the cheapest off-trail connector.
#[derive(Debug, Clone)]
pub struct OffTrailConnector {
    pub graph_node_id: i64,
    pub mesh_node_id: MeshNodeId,
    pub path: PathResult,
}

/// Run Theta\* from `start` to every exit in turn; return the result
/// with the lowest path length.
///
/// Why not run a single multi-target search: Theta\*'s admissibility
/// depends on the goal being a single point (the heuristic is
/// Euclidean distance to that goal). Multi-goal A\*/Theta\* needs a
/// "minimum-of" heuristic and a closed-set adjustment that's more
/// error-prone than just running k single-target searches at small
/// k. With ≤10 exits per query, the cost is fine.
///
/// Returns `None` if no exit is reachable. The caller decides whether
/// to widen the bbox or report "too far from any trail".
pub fn nearest_exit_via_mesh(built: &BuiltMesh) -> Option<OffTrailConnector> {
    let start = built.start_node?;
    let mut best: Option<OffTrailConnector> = None;
    for (&graph_id, &exit_mid) in &built.exits {
        let Some(path) = theta_star(&built.mesh, start, exit_mid) else {
            continue;
        };
        let current_best_len = best
            .as_ref()
            .map(|b| b.path.length_m)
            .unwrap_or(f64::INFINITY);
        if path.length_m < current_best_len {
            best = Some(OffTrailConnector {
                graph_node_id: graph_id,
                mesh_node_id: exit_mid,
                path,
            });
        }
    }
    best
}

/// Convenience: the connector's geometry in the same CRS as the mesh
/// (typically EPSG:25833 metres). Returns empty if no path.
pub fn connector_geometry(c: &OffTrailConnector) -> &[Point2] {
    &c.path.geometry
}

#[cfg(test)]
mod tests {
    use super::super::off_trail::{Mesh, MeshNode};
    use super::super::off_trail_mesh::{
        build_local_mesh, ExitNode, MeshBbox, MeshBuildInput, RefusedPolygon,
    };
    use super::*;

    fn pt(x: f64, y: f64) -> Point2 {
        Point2 { x, y }
    }

    fn small_bbox() -> MeshBbox {
        MeshBbox {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 500.0,
            max_y: 500.0,
        }
    }

    #[test]
    fn picks_the_only_reachable_exit() {
        // One exit, no obstacles, start in the middle. The connector
        // must return a non-empty path that ends at the exit mesh node.
        let built = build_local_mesh(MeshBuildInput {
            bbox: small_bbox(),
            cell_m: 100.0,
            start: Some(pt(50.0, 50.0)),
            exits: vec![ExitNode {
                graph_node_id: 7,
                at: pt(450.0, 450.0),
            }],
            ..Default::default()
        });
        let c = nearest_exit_via_mesh(&built).expect("connector found");
        assert_eq!(c.graph_node_id, 7);
        assert!(!c.path.geometry.is_empty());
        assert!(c.path.length_m > 0.0);
    }

    #[test]
    fn picks_the_closer_of_two_exits_on_open_terrain() {
        // Two exits at different distances. On a uniform mesh with
        // no obstacles the connector must pick the geometrically
        // closer one — but note BOTH exits get `cost_mul = 0.5`,
        // so the comparison is purely on path length.
        let built = build_local_mesh(MeshBuildInput {
            bbox: small_bbox(),
            cell_m: 100.0,
            start: Some(pt(50.0, 50.0)),
            exits: vec![
                ExitNode {
                    graph_node_id: 1,
                    at: pt(150.0, 50.0), // close
                },
                ExitNode {
                    graph_node_id: 2,
                    at: pt(450.0, 450.0), // far
                },
            ],
            ..Default::default()
        });
        let c = nearest_exit_via_mesh(&built).unwrap();
        assert_eq!(c.graph_node_id, 1, "expected near exit, got far one");
    }

    #[test]
    fn detours_around_refused_polygon_to_reach_exit() {
        // Refused square between start and exit. The connector path
        // must still find the exit, with length greater than the
        // straight-line distance.
        let bbox = small_bbox();
        let built = build_local_mesh(MeshBuildInput {
            bbox,
            cell_m: 100.0,
            start: Some(pt(50.0, 250.0)),
            exits: vec![ExitNode {
                graph_node_id: 99,
                at: pt(450.0, 250.0),
            }],
            refused: vec![RefusedPolygon {
                ring: vec![
                    pt(200.0, 100.0),
                    pt(300.0, 100.0),
                    pt(300.0, 400.0),
                    pt(200.0, 400.0),
                ],
            }],
            ..Default::default()
        });
        let c = nearest_exit_via_mesh(&built).expect("connector should still find the exit");
        let straight = pt(50.0, 250.0).dist(pt(450.0, 250.0));
        assert!(
            c.path.length_m > straight,
            "expected detour to exceed straight-line {}",
            straight
        );
        assert_eq!(c.graph_node_id, 99);
    }

    #[test]
    fn returns_none_when_no_exits() {
        // Empty exit set is a programmer error in practice (caller
        // should at least try to provide some nearby graph nodes),
        // but the connector must not panic.
        let built = build_local_mesh(MeshBuildInput {
            bbox: small_bbox(),
            cell_m: 100.0,
            start: Some(pt(250.0, 250.0)),
            ..Default::default()
        });
        assert!(nearest_exit_via_mesh(&built).is_none());
    }

    #[test]
    fn returns_none_when_no_start_node() {
        // build_local_mesh leaves start_node = None when caller didn't
        // pass a start. Connector must handle that gracefully — no
        // panic, no path.
        let built = build_local_mesh(MeshBuildInput {
            bbox: small_bbox(),
            cell_m: 100.0,
            exits: vec![ExitNode {
                graph_node_id: 1,
                at: pt(100.0, 100.0),
            }],
            ..Default::default()
        });
        assert!(nearest_exit_via_mesh(&built).is_none());
    }

    #[test]
    fn returns_none_when_exits_are_unreachable() {
        // Construct a mesh where the only path to the exit is blocked
        // by refused terrain that completely encircles the exit.
        // The exit gets cost_mul=0.5 but its neighbouring grid cells
        // are refused, so Theta* has no edge to traverse. The function
        // must return None instead of crashing.
        let nodes = vec![
            MeshNode {
                pt: pt(0.0, 0.0),
                cost_mul: 1.0,
            },
            MeshNode {
                pt: pt(100.0, 0.0),
                cost_mul: 0.5, // exit
            },
        ];
        // No edges between them — they're in separate components.
        let mesh = Mesh::new(nodes);
        let mut exits = std::collections::HashMap::new();
        exits.insert(42, MeshNodeId(1));
        let built = BuiltMesh {
            mesh,
            start_node: Some(MeshNodeId(0)),
            exits,
            grid_dims: (0, 0),
        };
        assert!(nearest_exit_via_mesh(&built).is_none());
    }
}
