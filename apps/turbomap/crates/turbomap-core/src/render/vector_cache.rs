//! GPU mesh cache for tessellated vector tiles. Bounded LRU mirroring the
//! raster `TextureCache` — keyed by `TileId`, each entry holds an
//! immutable vertex+index buffer pair.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use web_time::Instant;

use wgpu::util::DeviceExt;

use crate::{
    spatial_index::SpatialIndex,
    tessellate::{IconRequest, InteractiveFeature, LabelRequest, Mesh},
    tile::TileId,
};

pub(crate) struct VectorEntry {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    /// Water-body fills split out at tessellation time, drawn by the dedicated
    /// realistic-water pipeline. Separate vertex/index buffers (same vertex
    /// format as the main mesh). `water_index_count == 0` ⇒ no water in this tile
    /// (the buffers are 4-byte placeholders).
    pub water_vertex_buffer: wgpu::Buffer,
    pub water_index_buffer: wgpu::Buffer,
    pub water_index_count: u32,
    pub labels: Vec<LabelRequest>,
    pub icons: Vec<IconRequest>,
    pub interactive: Vec<InteractiveFeature>,
    /// Spatial index over `interactive` features keyed by tile-local
    /// AABB. `Map::hit_test` queries the cell under the cursor instead
    /// of walking every feature linearly — drops dense-tile hit-test
    /// from O(N) to ~O(N/256) average.
    pub hit_index: SpatialIndex,
    pub bytes: usize,
    /// Staged for vector tile fade-in (roadmap item #4).
    #[allow(dead_code)]
    pub created_at: Instant,
}

pub(crate) struct VectorMeshCache {
    entries: HashMap<TileId, VectorEntry>,
    lru: VecDeque<TileId>,
    bytes_used: usize,
    budget_bytes: usize,
    device: Arc<wgpu::Device>,
}

impl VectorMeshCache {
    pub(crate) fn new(device: Arc<wgpu::Device>, budget_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            lru: VecDeque::new(),
            bytes_used: 0,
            budget_bytes,
            device,
        }
    }

    /// Seconds since the tile was ingested, or `None` if not cached. The
    /// vector pipeline uses this to compute fade-in alpha.
    pub(crate) fn age_secs(&self, id: TileId) -> Option<f32> {
        self.entries
            .get(&id)
            .map(|e| Instant::now().duration_since(e.created_at).as_secs_f32())
    }

    pub(crate) fn any_younger_than(&self, max_age_secs: f32) -> bool {
        let now = Instant::now();
        self.entries
            .values()
            .any(|e| now.duration_since(e.created_at).as_secs_f32() < max_age_secs)
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn bytes_used(&self) -> usize {
        self.bytes_used
    }

    pub(crate) fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    /// Walk up the pyramid looking for an ancestor mesh we can scale
    /// to fill the requested tile while the real tile fetches. Bumps
    /// the ancestor in the LRU so it's not evicted out from under
    /// the pending fetch.
    pub(crate) fn nearest_ancestor(&mut self, id: TileId) -> Option<TileId> {
        for k in 1..=id.z {
            let ancestor = id.ancestor(k)?;
            if self.entries.contains_key(&ancestor) {
                self.touch(ancestor);
                return Some(ancestor);
            }
        }
        None
    }

    pub(crate) fn get(&mut self, id: TileId) -> Option<&VectorEntry> {
        if self.entries.contains_key(&id) {
            self.touch(id);
            self.entries.get(&id)
        } else {
            None
        }
    }

    /// Read-only lookup — does *not* bump the LRU. Used by hit-testing,
    /// which should not influence eviction order.
    pub(crate) fn peek(&self, id: TileId) -> Option<&VectorEntry> {
        self.entries.get(&id)
    }

    /// Insert a tessellated tile, evicting LRU tiles past budget. Returns
    /// the evicted ids so the caller can drop them from its "ingested"
    /// set — otherwise an evicted tile is never re-requested.
    pub(crate) fn insert(
        &mut self,
        id: TileId,
        mesh: &Mesh,
        water_mesh: &Mesh,
        labels: Vec<LabelRequest>,
        icons: Vec<IconRequest>,
        interactive: Vec<InteractiveFeature>,
    ) -> Vec<TileId> {
        if self.entries.contains_key(&id) {
            self.touch(id);
            return Vec::new();
        }

        // Build GPU buffers for both the ordinary vector mesh and the split-out
        // water mesh. Empty meshes get 4-byte placeholders with index_count 0, so
        // even a fully-empty tile still inserts a "loaded but empty" marker entry
        // (the host then won't keep re-fetching it) and the draw loop skips it.
        let (vertex_buffer, index_buffer, index_count, mesh_bytes) =
            make_mesh_buffers(&self.device, mesh, "turbomap-vector");
        let (water_vertex_buffer, water_index_buffer, water_index_count, water_bytes) =
            make_mesh_buffers(&self.device, water_mesh, "turbomap-water");

        let label_bytes: usize = labels.iter().map(|l| l.text.len() + 32).sum();
        let icon_bytes: usize = icons.iter().map(|i| i.sprite.len() + 24).sum();
        let interactive_bytes: usize = interactive
            .iter()
            .map(
                |i| i.source_layer.len() + 128, /* rough cap per feature */
            )
            .sum();
        // Build spatial index over the interactive features. All
        // features in a tile share the same `extent`, so pick from
        // the first feature; fall back to MVT's standard 4 096 if
        // empty (shouldn't happen — we already returned for empty
        // tiles above).
        let extent = interactive
            .first()
            .map(|f| f.extent)
            .unwrap_or(4096);
        let mut hit_index = SpatialIndex::new(extent);
        for (i, f) in interactive.iter().enumerate() {
            // Constant 4 tile-local-unit tolerance so thin lines /
            // point-features always land in at least one cell. The
            // real per-click tolerance is applied later in
            // `geometry_hit`.
            hit_index.insert(i as u32, &f.feature.geometry, 4.0);
        }
        hit_index.finish();
        let bytes = mesh_bytes
            + water_bytes
            + label_bytes
            + icon_bytes
            + interactive_bytes
            + hit_index.bytes();
        self.entries.insert(
            id,
            VectorEntry {
                vertex_buffer,
                index_buffer,
                index_count,
                water_vertex_buffer,
                water_index_buffer,
                water_index_count,
                labels,
                icons,
                interactive,
                hit_index,
                bytes,
                created_at: Instant::now(),
            },
        );
        self.lru.push_back(id);
        self.bytes_used += bytes;
        self.evict_to_budget()
    }

    fn touch(&mut self, id: TileId) {
        if let Some(pos) = self.lru.iter().position(|&t| t == id) {
            self.lru.remove(pos);
            self.lru.push_back(id);
        }
    }

    fn evict_to_budget(&mut self) -> Vec<TileId> {
        let mut evicted = Vec::new();
        while self.bytes_used > self.budget_bytes && self.lru.len() > 1 {
            let Some(victim) = self.lru.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&victim) {
                self.bytes_used = self.bytes_used.saturating_sub(entry.bytes);
                evicted.push(victim);
            }
        }
        evicted
    }
}

/// Build a vertex+index buffer pair for one mesh. wgpu rejects zero-sized
/// buffers, so an empty mesh gets 4-byte placeholders and an index count of 0
/// (the draw loop skips a slot whose count is 0). Returns
/// `(vertex_buffer, index_buffer, index_count, bytes)`.
fn make_mesh_buffers(
    device: &wgpu::Device,
    mesh: &Mesh,
    label: &str,
) -> (wgpu::Buffer, wgpu::Buffer, u32, usize) {
    if mesh.is_empty() {
        let vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-empty-vb"),
            size: 4,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let ib = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-empty-ib"),
            size: 4,
            usage: wgpu::BufferUsages::INDEX,
            mapped_at_creation: false,
        });
        return (vb, ib, 0, 8);
    }
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let bytes = mesh.vertices.len() * std::mem::size_of::<crate::tessellate::VectorVertex>()
        + mesh.indices.len() * 4;
    (vb, ib, mesh.indices.len() as u32, bytes)
}
