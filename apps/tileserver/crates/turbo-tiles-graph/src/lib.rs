//! Routing-graph primitive: CSR adjacency over a mmap'd binary
//! artifact + Dijkstra in-process.
//!
//! ## On-disk format v1
//!
//! ```text
//! [32 B] generic artifact header (kind=Graph, version=GRAPH_FORMAT_VERSION)
//! [32 B] GraphMeta:
//!          node_count: u32
//!          edge_count: u32
//!          profile_count: u32
//!          srid: u32          (always 25833 for now)
//!          reserved: [u8; 16]
//! [node_count × 8 B] Node positions: (x: f32, y: f32) in EPSG:25833.
//! [edge_count × 32 B] EdgeRecord (see struct).
//! [(node_count + 1) × 4 B] CSR offsets: edges leaving node i live in
//!                          edge_indices[offsets[i]..offsets[i+1]].
//! [edge_count × 4 B] CSR edge index table (u32).
//! [edge_count × profile_count × 4 B] Precomputed per-profile cost
//!                          (f32, infinite if profile forbids edge).
//! ```
//!
//! All multi-byte fields little-endian. The mmap is read-only.

use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use memmap2::Mmap;
use thiserror::Error;
use turbo_tiles_artifacts::{
    check_header, read_header, ArtifactError, ArtifactKind, HEADER_BYTES,
};

pub const GRAPH_FORMAT_VERSION: u32 = 1;
pub const GRAPH_GEOM_FORMAT_VERSION: u32 = 1;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    Foot = 0,
    Bicycle = 1,
    Ski = 2,
}
pub const PROFILE_COUNT: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct GraphMeta {
    pub node_count: u32,
    pub edge_count: u32,
    pub profile_count: u32,
    pub srid: u32,
}
pub const GRAPH_META_BYTES: usize = 32;

pub fn write_meta<W: Write>(w: &mut W, m: &GraphMeta) -> std::io::Result<()> {
    w.write_u32::<LittleEndian>(m.node_count)?;
    w.write_u32::<LittleEndian>(m.edge_count)?;
    w.write_u32::<LittleEndian>(m.profile_count)?;
    w.write_u32::<LittleEndian>(m.srid)?;
    w.write_all(&[0u8; 16])?;
    Ok(())
}

pub fn read_meta<R: Read>(r: &mut R) -> std::io::Result<GraphMeta> {
    let node_count = r.read_u32::<LittleEndian>()?;
    let edge_count = r.read_u32::<LittleEndian>()?;
    let profile_count = r.read_u32::<LittleEndian>()?;
    let srid = r.read_u32::<LittleEndian>()?;
    let mut _r = [0u8; 16];
    r.read_exact(&mut _r)?;
    Ok(GraphMeta {
        node_count,
        edge_count,
        profile_count,
        srid,
    })
}

/// 32-byte edge record. POD layout so mmap'd memory casts directly.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, serde::Serialize)]
pub struct EdgeRecord {
    pub from_id: u32,
    pub to_id: u32,
    pub length_m: f32,
    pub gain_m: f32,
    pub loss_m: f32,
    pub slope_max_deg: f32,
    pub fkb_type: u8,
    pub marking: u8,
    pub surface: u8,
    pub source: u8,
    pub attr_flags: u32,
}
pub const EDGE_RECORD_BYTES: usize = std::mem::size_of::<EdgeRecord>();

const _: () = assert!(EDGE_RECORD_BYTES == 32, "EdgeRecord must be exactly 32 bytes");

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, serde::Serialize)]
pub struct NodePos {
    pub x: f32,
    pub y: f32,
}

pub type NodeId = u32;
pub type EdgeId = u32;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("artifact: {0}")]
    Artifact(#[from] ArtifactError),
    #[error("malformed graph: {0}")]
    Malformed(&'static str),
    #[error("snap failed: no node within {radius_m} m of ({x:.1},{y:.1})")]
    SnapFailed { x: f64, y: f64, radius_m: f32 },
    #[error("invalid profile id: {0}")]
    InvalidProfile(u32),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphStats {
    pub meta: GraphMeta,
    pub file_size_bytes: u64,
    pub avg_edges_per_node: f32,
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RouteResult {
    pub edges: Vec<EdgeId>,
    pub nodes: Vec<NodeId>,
    pub length_m: f32,
    pub cost: f32,
}

/// Low-level Dijkstra exploration event for routing-replay
/// visualisation. Coordinates are EPSG:25833 metres (the same
/// projection the graph artifact stores). Pathfind translates
/// these to WGS84 on serialisation to the SPA.
#[derive(Debug, Clone, Copy)]
pub enum DijkstraEvent {
    /// A node was popped from the priority queue with cumulative
    /// cost `g`. The SPA renders pops as the expanding frontier.
    NodePopped { x: f32, y: f32, g: f32 },
    /// Edge from (fx, fy) to (tx, ty) was relaxed — i.e. the cost
    /// to reach `(tx, ty)` via this edge was lower than any
    /// previous path and the queue got a new entry.
    EdgeRelaxed { fx: f32, fy: f32, tx: f32, ty: f32, new_g: f32 },
}

pub struct Graph {
    _mmap: Mmap,
    meta: GraphMeta,
    file_size_bytes: u64,
    nodes: &'static [NodePos],
    edges: &'static [EdgeRecord],
    csr_offsets: &'static [u32],
    csr_edges: &'static [u32],
    costs: &'static [f32],
    /// rstar bulk-loaded at `open()` time so `snap()` is sub-50 µs
    /// instead of O(N) (~5 ms on 142 K nodes). Worth ~100 ms of
    /// startup for an interactive admin UI.
    rtree: rstar::RTree<SnapPoint>,
    /// Per-edge AABB rstar — used by the inspect overlay to find
    /// edges threading through a viewport even when both endpoints
    /// sit outside it. The node-position rstar above misses those
    /// because it only locates *nodes*. Built from the polyline
    /// geometry when `graph_geom` is attached; otherwise from the
    /// straight from→to segment. Lazily populated on first call to
    /// keep boot fast — see `edge_aabb_rtree()`.
    edge_rtree: std::sync::OnceLock<rstar::RTree<EdgeAabb>>,
    /// Optional sibling artifact: per-edge polyline geometry. When
    /// loaded, route reconstruction walks each edge's polyline
    /// instead of jumping straight between its endpoint nodes.
    /// Without it, on-graph routes look like straight-segment
    /// caricatures of the underlying trail.
    geom: Option<GraphGeom>,
}

/// Per-edge polyline geometry loaded from `norway.graph_geom`.
/// Held inside the `Graph` so route reconstruction can ask
/// `graph.edge_polyline(edge_id)` in one shot.
struct GraphGeom {
    _mmap: Mmap,
    /// Per directed edge: (vertex_offset, vertex_count). The vertex
    /// offset is a position into `vertices` (NOT bytes).
    index: &'static [GraphGeomIndexEntry],
    /// Flat (x, y) buffer; an edge's polyline is the slice
    /// `vertices[entry.offset..entry.offset + entry.count]`.
    vertices: &'static [NodePos],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GraphGeomIndexEntry {
    pub offset: u32,
    pub count: u32,
}
pub const GRAPH_GEOM_INDEX_BYTES: usize = std::mem::size_of::<GraphGeomIndexEntry>();
const _: () = assert!(GRAPH_GEOM_INDEX_BYTES == 8);

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct GraphGeomMeta {
    pub edge_count: u32,
    pub total_vertices: u32,
}
pub const GRAPH_GEOM_META_BYTES: usize = 8 + 24; // 4 + 4 + 24 reserved = 32

pub fn write_graph_geom_meta<W: Write>(w: &mut W, m: &GraphGeomMeta) -> std::io::Result<()> {
    w.write_u32::<LittleEndian>(m.edge_count)?;
    w.write_u32::<LittleEndian>(m.total_vertices)?;
    w.write_all(&[0u8; 24])?;
    Ok(())
}

pub fn read_graph_geom_meta<R: Read>(r: &mut R) -> std::io::Result<GraphGeomMeta> {
    let edge_count = r.read_u32::<LittleEndian>()?;
    let total_vertices = r.read_u32::<LittleEndian>()?;
    let mut _r = [0u8; 24];
    r.read_exact(&mut _r)?;
    Ok(GraphGeomMeta {
        edge_count,
        total_vertices,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SnapPoint {
    pos: [f32; 2],
    idx: u32,
}
impl rstar::Point for SnapPoint {
    type Scalar = f32;
    const DIMENSIONS: usize = 2;
    fn generate(mut g: impl FnMut(usize) -> Self::Scalar) -> Self {
        SnapPoint {
            pos: [g(0), g(1)],
            idx: u32::MAX,
        }
    }
    fn nth(&self, index: usize) -> Self::Scalar {
        self.pos[index]
    }
    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        &mut self.pos[index]
    }
}

/// AABB envelope for one directed edge, stored in `edge_rtree`.
/// `rstar::RTreeObject` returns the envelope so queries like
/// `locate_in_envelope_intersecting` return every edge whose AABB
/// touches the probe box — including edges that pass through the
/// probe without either endpoint inside.
#[derive(Debug, Clone, Copy, PartialEq)]
struct EdgeAabb {
    min: [f32; 2],
    max: [f32; 2],
    edge_id: u32,
}

impl rstar::RTreeObject for EdgeAabb {
    type Envelope = rstar::AABB<[f32; 2]>;
    fn envelope(&self) -> Self::Envelope {
        rstar::AABB::from_corners(self.min, self.max)
    }
}

impl Graph {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, GraphError> {
        let file = File::open(path.as_ref())?;
        let file_size_bytes = file.metadata()?.len();
        let mmap = unsafe { Mmap::map(&file)? };
        if mmap.len() < HEADER_BYTES + GRAPH_META_BYTES {
            return Err(GraphError::Malformed("file shorter than header+meta"));
        }
        let mut cursor = Cursor::new(&mmap[..]);
        let header = read_header(&mut cursor)?;
        check_header(&header, ArtifactKind::Graph, GRAPH_FORMAT_VERSION)?;
        let meta = read_meta(&mut cursor)?;
        let nc = meta.node_count as usize;
        let ec = meta.edge_count as usize;
        let pc = meta.profile_count as usize;
        let off_nodes = HEADER_BYTES + GRAPH_META_BYTES;
        let off_edges = off_nodes + nc * std::mem::size_of::<NodePos>();
        let off_offsets = off_edges + ec * EDGE_RECORD_BYTES;
        let off_csr_edges = off_offsets + (nc + 1) * 4;
        let off_costs = off_csr_edges + ec * 4;
        let end = off_costs + ec * pc * 4;
        if mmap.len() < end {
            return Err(GraphError::Malformed("file shorter than declared sections"));
        }
        // SAFETY: section length checks above; lifetimes constrained
        // by the `_mmap` field we own. The static transmute is the
        // standard mmap-borrows-itself pattern.
        let bytes: &[u8] = &mmap;
        let nodes: &[NodePos] = bytemuck::cast_slice(
            &bytes[off_nodes..off_nodes + nc * std::mem::size_of::<NodePos>()],
        );
        let edges: &[EdgeRecord] =
            bytemuck::cast_slice(&bytes[off_edges..off_edges + ec * EDGE_RECORD_BYTES]);
        let csr_offsets: &[u32] =
            bytemuck::cast_slice(&bytes[off_offsets..off_offsets + (nc + 1) * 4]);
        let csr_edges: &[u32] =
            bytemuck::cast_slice(&bytes[off_csr_edges..off_csr_edges + ec * 4]);
        let costs: &[f32] =
            bytemuck::cast_slice(&bytes[off_costs..off_costs + ec * pc * 4]);

        let nodes_static = unsafe { std::mem::transmute::<&[NodePos], &'static [NodePos]>(nodes) };
        let edges_static =
            unsafe { std::mem::transmute::<&[EdgeRecord], &'static [EdgeRecord]>(edges) };
        let csr_offsets_static =
            unsafe { std::mem::transmute::<&[u32], &'static [u32]>(csr_offsets) };
        let csr_edges_static =
            unsafe { std::mem::transmute::<&[u32], &'static [u32]>(csr_edges) };
        let costs_static = unsafe { std::mem::transmute::<&[f32], &'static [f32]>(costs) };

        // Bulk-load an rstar over node positions. ~80 ms for
        // 150 K nodes on a laptop — paid once at open() to make
        // every subsequent snap sub-50 µs.
        let snap_points: Vec<SnapPoint> = nodes_static
            .iter()
            .enumerate()
            .map(|(i, n)| SnapPoint {
                pos: [n.x, n.y],
                idx: i as u32,
            })
            .collect();
        let rtree = rstar::RTree::bulk_load(snap_points);

        Ok(Self {
            _mmap: mmap,
            meta,
            file_size_bytes,
            nodes: nodes_static,
            edges: edges_static,
            csr_offsets: csr_offsets_static,
            csr_edges: csr_edges_static,
            costs: costs_static,
            rtree,
            edge_rtree: std::sync::OnceLock::new(),
            geom: None,
        })
    }

    /// Build (or return) the edge-AABB rstar. Lazy + idempotent:
    /// the first inspect call pays the build cost (~200–500 ms for
    /// 2.7 M edges on the laptop); every later call is O(1).
    /// Constructed from the per-edge polyline when `graph_geom` is
    /// attached so the AABB tightly bounds the actual trail shape,
    /// otherwise from the straight from→to segment.
    fn edge_aabb_rtree(&self) -> &rstar::RTree<EdgeAabb> {
        self.edge_rtree.get_or_init(|| {
            let mut entries: Vec<EdgeAabb> = Vec::with_capacity(self.edges.len());
            for (i, er) in self.edges.iter().enumerate() {
                let poly = self.edge_polyline(i as u32);
                if poly.is_empty() {
                    continue;
                }
                let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
                let (mut max_x, mut max_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
                for p in &poly {
                    if p.x < min_x { min_x = p.x; }
                    if p.y < min_y { min_y = p.y; }
                    if p.x > max_x { max_x = p.x; }
                    if p.y > max_y { max_y = p.y; }
                }
                let _ = er;
                entries.push(EdgeAabb {
                    min: [min_x, min_y],
                    max: [max_x, max_y],
                    edge_id: i as u32,
                });
            }
            rstar::RTree::bulk_load(entries)
        })
    }

    /// Attach the sibling `norway.graph_geom` artifact, supplying
    /// per-edge polyline geometry. Call after `open()` once the
    /// caller has located the file; failure (missing or malformed)
    /// is non-fatal — routes just fall back to endpoint-segment
    /// geometry. Returns `true` on success.
    pub fn attach_geom<P: AsRef<Path>>(&mut self, path: P) -> Result<bool, GraphError> {
        let file = File::open(path.as_ref())?;
        let mmap = unsafe { Mmap::map(&file)? };
        if mmap.len() < HEADER_BYTES + GRAPH_GEOM_META_BYTES {
            return Err(GraphError::Malformed("geom file shorter than header+meta"));
        }
        let mut cursor = Cursor::new(&mmap[..]);
        let header = read_header(&mut cursor)?;
        check_header(&header, ArtifactKind::GraphGeom, GRAPH_GEOM_FORMAT_VERSION)?;
        let meta = read_graph_geom_meta(&mut cursor)?;
        if meta.edge_count as usize != self.edges.len() {
            return Err(GraphError::Malformed(
                "geom edge_count doesn't match graph artifact",
            ));
        }
        let ec = meta.edge_count as usize;
        let vc = meta.total_vertices as usize;
        let off_index = HEADER_BYTES + GRAPH_GEOM_META_BYTES;
        let off_verts = off_index + ec * GRAPH_GEOM_INDEX_BYTES;
        let end = off_verts + vc * std::mem::size_of::<NodePos>();
        if mmap.len() < end {
            return Err(GraphError::Malformed("geom file shorter than declared sections"));
        }
        // SAFETY: bounds checked above; mmap owned by `GraphGeom`.
        let bytes: &[u8] = &mmap;
        let index: &[GraphGeomIndexEntry] =
            bytemuck::cast_slice(&bytes[off_index..off_index + ec * GRAPH_GEOM_INDEX_BYTES]);
        let verts: &[NodePos] = bytemuck::cast_slice(
            &bytes[off_verts..off_verts + vc * std::mem::size_of::<NodePos>()],
        );
        let index_static =
            unsafe { std::mem::transmute::<&[GraphGeomIndexEntry], &'static [GraphGeomIndexEntry]>(index) };
        let verts_static = unsafe { std::mem::transmute::<&[NodePos], &'static [NodePos]>(verts) };
        self.geom = Some(GraphGeom {
            _mmap: mmap,
            index: index_static,
            vertices: verts_static,
        });
        Ok(true)
    }

    /// Polyline (vertex sequence) for a directed edge. Returns the
    /// straight `[from_node, to_node]` 2-point line when no
    /// `graph_geom` artifact is attached.
    pub fn edge_polyline(&self, edge_id: EdgeId) -> Vec<NodePos> {
        if let Some(g) = self.geom.as_ref() {
            let idx = edge_id as usize;
            if idx < g.index.len() {
                let entry = &g.index[idx];
                let start = entry.offset as usize;
                let count = entry.count as usize;
                if count >= 2 && start + count <= g.vertices.len() {
                    return g.vertices[start..start + count].to_vec();
                }
            }
        }
        // Fallback: straight line between endpoints.
        let er = match self.edges.get(edge_id as usize) {
            Some(e) => e,
            None => return Vec::new(),
        };
        match (self.node(er.from_id), self.node(er.to_id)) {
            (Some(a), Some(b)) => vec![a, b],
            _ => Vec::new(),
        }
    }

    pub fn has_geom(&self) -> bool {
        self.geom.is_some()
    }

    pub fn meta(&self) -> &GraphMeta {
        &self.meta
    }
    pub fn node(&self, id: NodeId) -> Option<NodePos> {
        self.nodes.get(id as usize).copied()
    }

    /// Return edges whose endpoints fall within the EPSG:25833 bbox.
    /// `fkb_filter` of `None` accepts all edges; otherwise only those
    /// whose `fkb_type` is in the slice are returned. The result is
    /// hard-capped at `max_count`; when more edges would qualify,
    /// the iterator stride-skips to keep coverage roughly uniform.
    ///
    /// Returns each match as ((x1, y1), (x2, y2), fkb_type) so the
    /// caller can colour by surface type.
    pub fn edges_in_bbox(
        &self,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        fkb_filter: Option<&[u8]>,
        max_count: usize,
    ) -> Vec<((f32, f32), (f32, f32), u8)> {
        // rstar `AABB<T>` corners must be `T` not `[f32; 2]` — the
        // SnapPoint envelope is built from two phantom SnapPoints.
        let lo = SnapPoint {
            pos: [min_x as f32, min_y as f32],
            idx: u32::MAX,
        };
        let hi = SnapPoint {
            pos: [max_x as f32, max_y as f32],
            idx: u32::MAX,
        };
        let probe_aabb = rstar::AABB::from_corners(lo, hi);
        // Find nodes in the bbox via the existing rtree, then collect
        // their incident edges. Misses edges that thread *through* the
        // bbox without either endpoint inside, but those are rare at
        // sensible zoom levels (an edge is typically ~100 m).
        let mut seen_edge: std::collections::HashSet<u32> =
            std::collections::HashSet::new();
        let mut out: Vec<((f32, f32), (f32, f32), u8)> = Vec::new();
        let mut total_candidates = 0usize;
        for node in self.rtree.locate_in_envelope_intersecting(&probe_aabb) {
            let u = node.idx as usize;
            let s = self.csr_offsets[u] as usize;
            let e = self.csr_offsets[u + 1] as usize;
            for &eidx in &self.csr_edges[s..e] {
                total_candidates += 1;
                if !seen_edge.insert(eidx) {
                    continue;
                }
                let er = &self.edges[eidx as usize];
                if let Some(filter) = fkb_filter {
                    if !filter.contains(&er.fkb_type) {
                        continue;
                    }
                }
                let from = self.nodes[er.from_id as usize];
                let to = self.nodes[er.to_id as usize];
                out.push(((from.x, from.y), (to.x, to.y), er.fkb_type));
            }
        }
        let _ = total_candidates;
        // Stride-sample if oversized.
        if out.len() > max_count {
            let stride = (out.len() / max_count).max(1);
            out.into_iter().step_by(stride).take(max_count).collect()
        } else {
            out
        }
    }

    /// Directed edge ids whose `from` node falls in `bbox` — the
    /// routing-oriented counterpart of [`Self::edges_in_bbox`] (which
    /// returns geometry for rendering). The unified solver uses this to
    /// splice the trail network inside a corridor into its node graph:
    /// for each id, `self.edge(id)` gives the `EdgeRecord` (for the
    /// walk-seconds cost) and `self.node(er.from_id/to_id)` the
    /// endpoints. Directed (both orientations kept) so adjacency is
    /// built directly. Capped at `max_count` (stride-sampled) so a
    /// dense bbox can't explode the node graph.
    pub fn edge_ids_in_bbox(
        &self,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        max_count: usize,
    ) -> Vec<EdgeId> {
        let lo = SnapPoint { pos: [min_x as f32, min_y as f32], idx: u32::MAX };
        let hi = SnapPoint { pos: [max_x as f32, max_y as f32], idx: u32::MAX };
        let probe_aabb = rstar::AABB::from_corners(lo, hi);
        let mut out: Vec<EdgeId> = Vec::new();
        for node in self.rtree.locate_in_envelope_intersecting(&probe_aabb) {
            let u = node.idx as usize;
            let s = self.csr_offsets[u] as usize;
            let e = self.csr_offsets[u + 1] as usize;
            for &eidx in &self.csr_edges[s..e] {
                out.push(eidx);
            }
        }
        if out.len() > max_count {
            let stride = (out.len() / max_count).max(1);
            out.into_iter().step_by(stride).take(max_count).collect()
        } else {
            out
        }
    }

    /// Polylines (NOT node-to-node secants) for every directed edge
    /// whose endpoints fall in `bbox`. Returns the full vertex
    /// sequence from `graph_geom` when attached, falling back to a
    /// 2-point line between endpoint nodes otherwise.
    ///
    /// Dedupes undirected edges via the canonical `(min(u,v),
    /// max(u,v))` pair so a single trail appears once, not once per
    /// direction. Stride-samples to `max_count` when the bbox is
    /// dense.
    ///
    /// Used by the admin SPA's trail overlay so the curator sees
    /// the *actual* trail shape — without this the overlay drew
    /// straight lines between graph junctions, making winding sti
    /// rows look like trail edges crossing water and disconnected
    /// segments. Routing was always correct; the overlay lied.
    pub fn edge_polylines_in_bbox(
        &self,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        fkb_filter: Option<&[u8]>,
        max_count: usize,
    ) -> Vec<(Vec<NodePos>, u8)> {
        // Use the per-edge AABB rstar (lazily built) so we find
        // every edge whose polyline AABB intersects the viewport,
        // including long edges that thread through small high-zoom
        // bboxes without either endpoint inside. The old approach
        // queried the node rstar and walked incident edges, which
        // misses precisely those threading edges — the bug behind
        // Panel 5's empty zoom-15 overlay.
        let probe = rstar::AABB::from_corners(
            [min_x as f32, min_y as f32],
            [max_x as f32, max_y as f32],
        );
        let mut seen_pair: std::collections::HashSet<(u32, u32)> =
            std::collections::HashSet::new();
        let mut out: Vec<(Vec<NodePos>, u8)> = Vec::new();
        for ent in self.edge_aabb_rtree().locate_in_envelope_intersecting(&probe) {
            let eidx = ent.edge_id;
            let er = &self.edges[eidx as usize];
            if let Some(filter) = fkb_filter {
                if !filter.contains(&er.fkb_type) {
                    continue;
                }
            }
            // Dedupe forward + reverse direction so each undirected
            // edge appears once. The graph builder writes both
            // directions for every road/trail.
            let pair = if er.from_id <= er.to_id {
                (er.from_id, er.to_id)
            } else {
                (er.to_id, er.from_id)
            };
            if !seen_pair.insert(pair) {
                continue;
            }
            let poly = self.edge_polyline(eidx);
            if poly.len() >= 2 {
                out.push((poly, er.fkb_type));
            }
        }
        if out.len() > max_count {
            let stride = (out.len() / max_count).max(1);
            out.into_iter().step_by(stride).take(max_count).collect()
        } else {
            out
        }
    }

    /// Iterator over (a, b) segment endpoints (in EPSG:25833 m) for
    /// every edge whose `fkb_type` matches one of `kinds`. Pass an
    /// empty slice to get every edge.
    ///
    /// Used by `TrailProximityLayer` (in `turbo-tiles-pathfind`) to
    /// build a spatial index over the trail network so the off-
    /// trail solver can bias cells toward existing trails.
    pub fn collect_segments_with_fkb_types(
        &self,
        kinds: &[u8],
    ) -> Vec<((f32, f32), (f32, f32))> {
        self.edges
            .iter()
            .filter(|e| kinds.is_empty() || kinds.contains(&e.fkb_type))
            .filter_map(|e| {
                let a = self.node(e.from_id)?;
                let b = self.node(e.to_id)?;
                Some(((a.x, a.y), (b.x, b.y)))
            })
            .collect()
    }

    /// Sampled subset of nodes for "show me where the graph has
    /// data" admin overlays. With 150 K nodes a faithful render is
    /// noise; a stride-sampled set of `~target` items conveys
    /// density just as well at 0.1% the GeoJSON size.
    pub fn sample_nodes(&self, target: usize) -> Vec<NodePos> {
        if self.nodes.is_empty() {
            return Vec::new();
        }
        let stride = (self.nodes.len() / target.max(1)).max(1);
        self.nodes.iter().step_by(stride).copied().collect()
    }
    pub fn edge(&self, id: EdgeId) -> Option<&EdgeRecord> {
        self.edges.get(id as usize)
    }

    pub fn stats(&self) -> GraphStats {
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for n in self.nodes {
            min_x = min_x.min(n.x);
            min_y = min_y.min(n.y);
            max_x = max_x.max(n.x);
            max_y = max_y.max(n.y);
        }
        let avg = if self.meta.node_count > 0 {
            self.meta.edge_count as f32 / self.meta.node_count as f32
        } else {
            0.0
        };
        GraphStats {
            meta: self.meta,
            file_size_bytes: self.file_size_bytes,
            avg_edges_per_node: avg,
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    /// Snap a point to the nearest node within `radius_m`.
    ///
    /// rstar nearest-neighbour iteration — typically sub-50 µs even
    /// on 1 M nodes. Returns `SnapFailed` when the closest node is
    /// still farther than `radius_m`.
    pub fn snap(&self, x: f64, y: f64, radius_m: f32) -> Result<NodeId, GraphError> {
        let probe = SnapPoint {
            pos: [x as f32, y as f32],
            idx: u32::MAX,
        };
        match self.rtree.nearest_neighbor(&probe) {
            Some(p) => {
                let dx = p.pos[0] - x as f32;
                let dy = p.pos[1] - y as f32;
                let d = (dx * dx + dy * dy).sqrt();
                if d > radius_m {
                    Err(GraphError::SnapFailed { x, y, radius_m })
                } else {
                    Ok(p.idx)
                }
            }
            None => Err(GraphError::SnapFailed { x, y, radius_m }),
        }
    }

    /// Dijkstra from `from` → `to` using the precomputed cost table.
    pub fn route(
        &self,
        from: NodeId,
        to: NodeId,
        profile: Profile,
    ) -> Result<Option<RouteResult>, GraphError> {
        // Default routing uses the build-time baked cost directly.
        self.route_with(from, to, profile, |_eid, _er, baked| baked)
    }

    /// Same as [`route_with`] but emits a stream of low-level events
    /// to the observer closure — one per popped node, one per
    /// relaxed edge. The pathfind crate uses this to feed its
    /// per-event recording for the SPA's algorithm-replay overlay.
    /// The observer runs synchronously inside the Dijkstra loop, so
    /// it must be cheap; the pathfind side pushes to a thread-local
    /// recorder that short-circuits when no recording is active.
    pub fn route_with_observer<F, O>(
        &self,
        from: NodeId,
        to: NodeId,
        profile: Profile,
        edge_cost: F,
        on_event: O,
    ) -> Result<Option<RouteResult>, GraphError>
    where
        F: Fn(EdgeId, &EdgeRecord, f32) -> f32,
        O: Fn(DijkstraEvent),
    {
        self.route_inner(from, to, profile, edge_cost, Some(on_event))
    }

    /// Dijkstra with a per-edge ABSOLUTE-cost closure. The closure
    /// receives `(edge_id, &EdgeRecord, baked_cost)` and returns the
    /// edge's traversal cost (the pathfinder composes honest per-metre
    /// walk-seconds along the edge's real polyline). The `baked_cost`
    /// argument carries the build-time per-profile cost so the closure
    /// can honour the profile-forbidden flag (`+inf`) or fall back to
    /// it. Returns `f32::INFINITY` for forbidden edges.
    pub fn route_with<F>(
        &self,
        from: NodeId,
        to: NodeId,
        profile: Profile,
        edge_cost: F,
    ) -> Result<Option<RouteResult>, GraphError>
    where
        F: Fn(EdgeId, &EdgeRecord, f32) -> f32,
    {
        self.route_inner::<F, fn(DijkstraEvent)>(from, to, profile, edge_cost, None)
    }

    fn route_inner<F, O>(
        &self,
        from: NodeId,
        to: NodeId,
        profile: Profile,
        edge_cost: F,
        on_event: Option<O>,
    ) -> Result<Option<RouteResult>, GraphError>
    where
        F: Fn(EdgeId, &EdgeRecord, f32) -> f32,
        O: Fn(DijkstraEvent),
    {
        if from as usize >= self.nodes.len() || to as usize >= self.nodes.len() {
            return Err(GraphError::Malformed("node id out of range"));
        }
        let prof_id = profile as u32;
        if prof_id >= self.meta.profile_count {
            return Err(GraphError::InvalidProfile(prof_id));
        }
        let n = self.nodes.len();
        let mut dist: Vec<f32> = vec![f32::INFINITY; n];
        let mut prev_edge: Vec<i32> = vec![-1; n];
        let mut prev_node: Vec<i32> = vec![-1; n];
        dist[from as usize] = 0.0;

        use std::cmp::Ordering;
        use std::collections::BinaryHeap;
        #[derive(Copy, Clone)]
        struct OpenEntry(f32, u32);
        impl PartialEq for OpenEntry {
            fn eq(&self, o: &Self) -> bool {
                self.0 == o.0
            }
        }
        impl Eq for OpenEntry {}
        impl PartialOrd for OpenEntry {
            fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
                Some(self.cmp(o))
            }
        }
        impl Ord for OpenEntry {
            // BinaryHeap is max-heap → invert so smallest cost pops first.
            fn cmp(&self, o: &Self) -> Ordering {
                o.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
            }
        }
        let mut open: BinaryHeap<OpenEntry> = BinaryHeap::new();
        open.push(OpenEntry(0.0, from));
        while let Some(OpenEntry(d, u)) = open.pop() {
            if let Some(obs) = on_event.as_ref() {
                let np = self.nodes[u as usize];
                obs(DijkstraEvent::NodePopped { x: np.x, y: np.y, g: d });
            }
            if u == to {
                break;
            }
            if d > dist[u as usize] {
                continue;
            }
            let s = self.csr_offsets[u as usize] as usize;
            let e = self.csr_offsets[u as usize + 1] as usize;
            for &eidx in &self.csr_edges[s..e] {
                let er = &self.edges[eidx as usize];
                let cost_idx = eidx as usize * self.meta.profile_count as usize
                    + prof_id as usize;
                // `baked` is the build-time per-profile cost in
                // "effective metres"; it is `+inf` when the profile
                // forbids the edge. We no longer multiply by it —
                // the closure returns the edge's ABSOLUTE cost (the
                // pathfinder composes honest per-metre walk-seconds
                // along the real polyline) — but we still pass it
                // through so the closure can honour the forbidden
                // flag and the default `route()` can fall back to it.
                let baked = self.costs[cost_idx];
                let w = edge_cost(eidx, er, baked);
                if !w.is_finite() {
                    continue;
                }
                let nd = d + w;
                let v = er.to_id;
                if nd < dist[v as usize] {
                    dist[v as usize] = nd;
                    prev_edge[v as usize] = eidx as i32;
                    prev_node[v as usize] = u as i32;
                    if let Some(obs) = on_event.as_ref() {
                        let np_u = self.nodes[u as usize];
                        let np_v = self.nodes[v as usize];
                        obs(DijkstraEvent::EdgeRelaxed {
                            fx: np_u.x,
                            fy: np_u.y,
                            tx: np_v.x,
                            ty: np_v.y,
                            new_g: nd,
                        });
                    }
                    open.push(OpenEntry(nd, v));
                }
            }
        }
        if !dist[to as usize].is_finite() {
            return Ok(None);
        }
        let mut edges = Vec::new();
        let mut nodes = vec![to];
        let mut cur = to as i32;
        while cur != from as i32 {
            let e = prev_edge[cur as usize];
            if e < 0 {
                return Err(GraphError::Malformed("path reconstruction broken"));
            }
            edges.push(e as u32);
            cur = prev_node[cur as usize];
            nodes.push(cur as u32);
        }
        edges.reverse();
        nodes.reverse();
        let length_m: f32 = edges
            .iter()
            .map(|&eid| self.edges[eid as usize].length_m)
            .sum();
        Ok(Some(RouteResult {
            edges,
            nodes,
            length_m,
            cost: dist[to as usize],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_record_byte_size_locked() {
        assert_eq!(EDGE_RECORD_BYTES, 32);
    }

    #[test]
    fn meta_round_trip() {
        let m = GraphMeta {
            node_count: 10,
            edge_count: 20,
            profile_count: 3,
            srid: 25833,
        };
        let mut buf = Vec::new();
        write_meta(&mut buf, &m).unwrap();
        assert_eq!(buf.len(), GRAPH_META_BYTES);
        let parsed = read_meta(&mut &buf[..]).unwrap();
        assert_eq!(parsed, m);
    }
}
