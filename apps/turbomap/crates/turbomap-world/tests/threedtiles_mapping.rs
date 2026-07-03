//! The D3 design-validation gate (plan slice B1): a real-shaped 3D Tiles
//! `tileset.json` must map LOSSLESSLY onto `ChunkMeta`/`NodeId`/tree types
//! **before the types freeze** — proving the chunk model isn't secretly a
//! quadtree with extra steps. This test is the mapping's executable spec; the
//! actual tileset codec ships later, behind milestone M-3DTILES.
//!
//! What "lossless" must cover (each is something a quadtree-shaped model
//! would silently drop):
//! - all three bounding-volume forms (`region`, `box`, `sphere`), bit-exact;
//! - per-node `geometricError` in meters, not implied by depth;
//! - `refine` REPLACE/ADD with 3D Tiles inheritance (absent → inherit from
//!   the nearest ancestor that declared it);
//! - arbitrary branching + depth (not 4 children, not level-aligned);
//! - nodes with and without content (interior organizational nodes).

use serde_json::Value;
use turbomap_world::{BoundingVolume, ChunkKey, ChunkMeta, NodeId, Refine, WorldLayerId};

/// The arena an explicit tree's fetched pages populate: one entry per node,
/// `NodeId` = arena index. This is the shape the M-3DTILES codec will emit.
struct ExplicitNode {
    meta: ChunkMeta,
    content_uri: Option<String>,
    children: Vec<NodeId>,
}

/// Map a parsed tileset tile (and its subtree) into the arena — the reference
/// mapping the future codec must reproduce.
fn map_tile(tile: &Value, inherited_refine: Refine, arena: &mut Vec<ExplicitNode>) -> NodeId {
    let bv = &tile["boundingVolume"];
    let bounds = if let Some(r) = bv["region"].as_array() {
        let v: Vec<f64> = r.iter().map(|x| x.as_f64().unwrap()).collect();
        BoundingVolume::Region {
            west: v[0],
            south: v[1],
            east: v[2],
            north: v[3],
            min_height_m: v[4],
            max_height_m: v[5],
        }
    } else if let Some(b) = bv["box"].as_array() {
        let mut a = [0.0; 12];
        for (i, x) in b.iter().enumerate() {
            a[i] = x.as_f64().unwrap();
        }
        BoundingVolume::Box3(a)
    } else if let Some(s) = bv["sphere"].as_array() {
        let mut a = [0.0; 4];
        for (i, x) in s.iter().enumerate() {
            a[i] = x.as_f64().unwrap();
        }
        BoundingVolume::Sphere(a)
    } else {
        panic!("tileset tile without a supported boundingVolume: {bv}");
    };

    // 3D Tiles refine semantics: absent inherits from the nearest ancestor.
    let refine = match tile["refine"].as_str() {
        Some("REPLACE") => Refine::Replace,
        Some("ADD") => Refine::Add,
        Some(other) => panic!("unknown refine {other}"),
        None => inherited_refine,
    };

    let id = NodeId(arena.len() as u64);
    arena.push(ExplicitNode {
        meta: ChunkMeta {
            bounds,
            geometric_error_m: tile["geometricError"].as_f64().expect("geometricError"),
            refine,
        },
        content_uri: tile["content"]["uri"].as_str().map(str::to_owned),
        children: Vec::new(),
    });

    let child_ids: Vec<NodeId> = tile["children"]
        .as_array()
        .map(|cs| cs.iter().map(|c| map_tile(c, refine, arena)).collect())
        .unwrap_or_default();
    arena[id.0 as usize].children = child_ids;
    id
}

#[test]
fn a_real_shaped_tileset_maps_losslessly_onto_the_chunk_model() {
    let json: Value =
        serde_json::from_str(include_str!("fixtures/tileset.json")).expect("valid fixture JSON");

    let mut arena = Vec::new();
    // Spec: a root without `refine` defaults to REPLACE.
    let root = map_tile(&json["root"], Refine::Replace, &mut arena);

    // Shape: 5 nodes, non-quadtree branching (2 children, then 1 and 1).
    assert_eq!(arena.len(), 5);
    assert_eq!(arena[root.0 as usize].children.len(), 2);

    // Every node addresses as a ChunkKey like any pyramid tile would — the
    // streaming table is oblivious to which tree shape minted the NodeId.
    let layer = WorldLayerId(7);
    let keys: Vec<ChunkKey> = (0..arena.len() as u64)
        .map(|n| ChunkKey {
            layer,
            node: NodeId(n),
        })
        .collect();
    assert_eq!(keys.len(), 5);

    // Bounding volumes: all three forms survived bit-exactly.
    let BoundingVolume::Region { west, north, max_height_m, .. } = arena[root.0 as usize].meta.bounds
    else {
        panic!("root is a region");
    };
    assert_eq!(west, 0.0929);
    assert_eq!(north, 1.0541);
    assert_eq!(max_height_m, 120.0);

    let first_child = arena[root.0 as usize].children[0];
    let BoundingVolume::Box3(bx) = arena[first_child.0 as usize].meta.bounds else {
        panic!("first child is an oriented box");
    };
    assert_eq!(&bx[..3], &[1.5, -2.0, 40.0]);
    assert_eq!(bx[3], 30.0);

    let grandchild = arena[first_child.0 as usize].children[0];
    let BoundingVolume::Sphere(sp) = arena[grandchild.0 as usize].meta.bounds else {
        panic!("grandchild is a sphere");
    };
    assert_eq!(sp, [1.5, -2.0, 60.0, 25.0]);

    // Geometric error is per-node data, NOT a function of depth: two nodes at
    // the same depth carry the same declared error, and a leaf may declare 0.
    let second_child = arena[root.0 as usize].children[1];
    assert_eq!(arena[first_child.0 as usize].meta.geometric_error_m, 64.0);
    assert_eq!(arena[second_child.0 as usize].meta.geometric_error_m, 64.0);
    assert_eq!(arena[grandchild.0 as usize].meta.geometric_error_m, 0.0);
    // And it never increases toward the leaves along any path (refinement).
    fn errors_nonincreasing(arena: &[ExplicitNode], id: NodeId) {
        let e = arena[id.0 as usize].meta.geometric_error_m;
        for &c in &arena[id.0 as usize].children {
            assert!(arena[c.0 as usize].meta.geometric_error_m <= e);
            errors_nonincreasing(arena, c);
        }
    }
    errors_nonincreasing(&arena, root);

    // Refine inheritance: root declares REPLACE; the box child inherits it;
    // the sphere grandchild overrides with ADD; the east branch inherits
    // REPLACE all the way down.
    assert_eq!(arena[first_child.0 as usize].meta.refine, Refine::Replace);
    assert_eq!(arena[grandchild.0 as usize].meta.refine, Refine::Add);
    let east_leaf = arena[second_child.0 as usize].children[0];
    assert_eq!(arena[east_leaf.0 as usize].meta.refine, Refine::Replace);

    // Content is optional per node: the east interior node is organizational
    // (no payload), its leaf carries one — both are representable.
    assert!(arena[second_child.0 as usize].content_uri.is_none());
    assert_eq!(
        arena[east_leaf.0 as usize].content_uri.as_deref(),
        Some("east/leaf.glb")
    );
    assert_eq!(arena[root.0 as usize].content_uri.as_deref(), Some("root.glb"));
}
