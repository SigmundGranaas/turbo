//! Generic vector feature store: many named collections in one
//! mmap'd artifact, each typed `Point | LineString | Polygon`.
//!
//! This crate replaces the "rasterise everything into a refusal
//! mask" pattern with vector primitives that preserve original
//! geometry. Cost layers (water crossing, stream crossing, cliff
//! avoidance, wetland traversal, …) ask geometric questions
//! against the relevant collection — "how many metres of this
//! proposed edge lie inside a water polygon?", "does this edge
//! cross a stream, and how wide is it?" — and feed the answer
//! through a per-layer cost function.
//!
//! ## On-disk layout
//!
//! ```text
//! [32 B] generic artifact header (kind=Vectors, version=VECTOR_FORMAT_VERSION)
//! [32 B] header tail:
//!          collection_count: u32
//!          manifest_offset:  u64
//!          manifest_len:     u64
//!          reserved:         u8 × 12
//! [manifest_len B] JSON manifest:
//!          [{ name, kind, feature_count, attr_schema, offset, byte_len }, …]
//! [per-collection blob …]
//! ```
//!
//! Manifest is JSON so new collections / schemas don't require
//! a format-version bump. The per-collection blobs are typed binary.
//!
//! ## Per-collection blob
//!
//! ```text
//! [4 B] feature_count: u32
//! [4 B] coord_count:   u32          (total points across all features)
//! [4 B] attrs_bytes:   u32          (total size of attribute blobs)
//! [feature_count × FeatureIndex]    (coord_offset, coord_count, attr_offset, aabb)
//! [coord_count × Point]             (flat f32 x, f32 y in EPSG:25833)
//! [attrs_bytes B]                   (concatenated attribute blobs)
//! ```
//!
//! The rstar AABB index is **rebuilt at `open()` time** from the
//! per-feature AABBs in the `FeatureIndex` array. `bulk_load` is
//! O(N log N) but ~10× faster than bincode-deserialising the tree
//! from disk for N≈10⁶, and removes a class of memory-corruption
//! failure modes when the serialized rstar bytes get aligned
//! awkwardly inside the mmap.
//!
//! All multi-byte fields little-endian. Coordinates are EPSG:25833
//! metres throughout the system.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Cursor, Write};
use std::path::Path;
use std::sync::Arc;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use memmap2::Mmap;
use rstar::{RTree, RTreeObject, AABB};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use turbo_tiles_artifacts::{
    check_header, read_header, write_header, ArtifactError, ArtifactKind, Header, HEADER_BYTES,
};
use turbo_tiles_geom::{Aabb, Point};

pub const VECTOR_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeomKind {
    Point,
    LineString,
    /// Single closed ring. Multi-ring polygons (e.g. with holes)
    /// are not modelled yet — the current cost layers don't care
    /// about holes, and N50 water/wetland data doesn't have them.
    Polygon,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttrSchema {
    /// Field definitions, in the byte order they appear in each
    /// feature's attribute blob. A reader uses this to decode
    /// attributes without baked-in knowledge of any collection.
    pub fields: Vec<AttrField>,
    /// Total byte length of one feature's blob — for O(1) random
    /// access by feature index.
    pub bytes_per_feature: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttrField {
    pub name: String,
    pub ty: AttrType,
    pub offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttrType {
    /// 4-byte little-endian f32 — the most common attribute shape
    /// (widths, areas, slopes, elevations).
    F32,
    /// 4-byte little-endian u32 — categorical codes, ids.
    U32,
    /// Single byte — booleans / small enums.
    U8,
}

impl AttrType {
    pub fn size(self) -> u32 {
        match self {
            AttrType::F32 | AttrType::U32 => 4,
            AttrType::U8 => 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionManifest {
    pub name: String,
    pub kind: GeomKind,
    pub feature_count: u32,
    pub attr_schema: AttrSchema,
    pub offset: u64,
    pub byte_len: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub collections: Vec<CollectionManifest>,
    pub build_timestamp_unix_sec: i64,
}

/// Per-feature header sitting at the front of a collection blob.
/// 32 bytes so we can mmap-cast straight into `&[FeatureIndex]`.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FeatureIndex {
    pub coord_offset: u32,
    pub coord_count: u32,
    pub attr_offset: u32,
    pub _pad: u32,
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}
pub const FEATURE_INDEX_BYTES: usize = std::mem::size_of::<FeatureIndex>();
const _: () = assert!(FEATURE_INDEX_BYTES == 32);

#[derive(Debug, Error)]
pub enum VectorError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("artifact: {0}")]
    Artifact(#[from] ArtifactError),
    #[error("malformed vectors artifact: {0}")]
    Malformed(&'static str),
    #[error("manifest json: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("collection '{0}' not found in vector store")]
    UnknownCollection(String),
    #[error("attr decode: field '{field}' missing or wrong type")]
    BadAttr { field: String },
    #[error("collection kind mismatch: '{name}' is {actual:?}, wanted {wanted:?}")]
    KindMismatch {
        name: String,
        actual: GeomKind,
        wanted: GeomKind,
    },
}

/// Bincode-serialised wrapper carrying the per-feature envelope into
/// `rstar`. The store owns the deserialised tree; queries hand back
/// raw u32 feature ids that index into the collection's flat arrays.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct IndexedFeature {
    aabb_min: [f32; 2],
    aabb_max: [f32; 2],
    feature_id: u32,
}

impl RTreeObject for IndexedFeature {
    type Envelope = AABB<[f32; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(self.aabb_min, self.aabb_max)
    }
}

/// One collection in the store. Borrowed views into the mmap'd
/// vector artifact plus the deserialised rstar.
///
/// Holds its own `Arc<Mmap>` so the mapping survives as long as
/// any collection — the parent [`VectorStore`] is just a directory
/// lookup, and callers (e.g. cost layers stashed inside a
/// `Pathfinder`) typically outlive it. Without the Arc, dropping
/// the store would unmap memory the `'static` slices still point
/// at, producing exactly the segfault we hit when the boot wiring
/// let `VectorStore` go out of scope.
pub struct VectorCollection {
    name: String,
    kind: GeomKind,
    attr_schema: AttrSchema,
    features: &'static [FeatureIndex],
    coords: &'static [Point],
    attrs: &'static [u8],
    rtree: RTree<IndexedFeature>,
    /// Lazily-built y-banded edge indexes for BIG polygon rings (see
    /// [`turbo_tiles_geom::RingIndex`]); keyed by feature id. Norway
    /// lake rings reach tens of thousands of vertices and the
    /// off-trail cost field intersects a tiny segment against them
    /// once per mesh cell — the index turns that O(ring) scan into a
    /// band lookup, bit-identically.
    ring_index: std::sync::Mutex<std::collections::HashMap<u32, Arc<turbo_tiles_geom::RingIndex>>>,
    /// Keeps the underlying mmap pages alive. Touched once per
    /// query path — the slices above already point at the pages.
    _mmap: Arc<Mmap>,
}

impl VectorCollection {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> GeomKind {
        self.kind
    }
    pub fn len(&self) -> usize {
        self.features.len()
    }
    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }
    pub fn schema(&self) -> &AttrSchema {
        &self.attr_schema
    }

    /// Coordinates of one feature: the ring of a polygon, the
    /// vertices of a polyline, or the single point.
    pub fn feature_coords(&self, idx: u32) -> &[Point] {
        let f = &self.features[idx as usize];
        let s = f.coord_offset as usize;
        let n = f.coord_count as usize;
        &self.coords[s..s + n]
    }

    /// Per-feature AABB.
    pub fn feature_aabb(&self, idx: u32) -> Aabb {
        let f = &self.features[idx as usize];
        Aabb {
            min_x: f.min_x,
            min_y: f.min_y,
            max_x: f.max_x,
            max_y: f.max_y,
        }
    }

    pub fn feature_attrs(&self, idx: u32) -> AttrView<'_> {
        let bytes_per = self.attr_schema.bytes_per_feature as usize;
        let f = &self.features[idx as usize];
        let start = f.attr_offset as usize;
        AttrView {
            schema: &self.attr_schema,
            blob: if bytes_per == 0 {
                &[]
            } else {
                &self.attrs[start..start + bytes_per]
            },
        }
    }

    /// Iterate features whose AABB intersects the given bbox.
    /// The geometric refinement (does the segment actually cross
    /// the polygon edge?) is the caller's responsibility — this
    /// is just a cheap pre-filter via rstar.
    pub fn query_aabb<'a>(&'a self, bbox: Aabb) -> impl Iterator<Item = u32> + 'a {
        let envelope = AABB::from_corners([bbox.min_x, bbox.min_y], [bbox.max_x, bbox.max_y]);
        self.rtree
            .locate_in_envelope_intersecting(&envelope)
            .map(|f| f.feature_id)
    }

    /// Y-banded edge index for feature `idx`'s ring, built lazily and
    /// cached. `None` for rings below the size threshold, where the
    /// brute scan is cheaper than the lookup.
    pub fn ring_index(&self, idx: u32) -> Option<Arc<turbo_tiles_geom::RingIndex>> {
        let coords = self.feature_coords(idx);
        if coords.len() < turbo_tiles_geom::RingIndex::MIN_RING_LEN {
            return None;
        }
        let mut cache = self.ring_index.lock().unwrap();
        Some(
            cache
                .entry(idx)
                .or_insert_with(|| Arc::new(turbo_tiles_geom::RingIndex::build(coords)))
                .clone(),
        )
    }

    /// Iterate features whose AABB could contain a crossing with
    /// the segment `[a, b]`. Computed by constructing a tight AABB
    /// around the segment (with optional padding) and using
    /// `query_aabb`. Geometric refinement is again the caller's.
    pub fn query_segment<'a>(
        &'a self,
        a: Point,
        b: Point,
        pad_m: f32,
    ) -> impl Iterator<Item = u32> + 'a {
        let bbox = Aabb {
            min_x: a.x.min(b.x) - pad_m,
            min_y: a.y.min(b.y) - pad_m,
            max_x: a.x.max(b.x) + pad_m,
            max_y: a.y.max(b.y) + pad_m,
        };
        self.query_aabb(bbox)
    }

    /// Features whose AABB lies within `radius_m` of `p`. Used by
    /// point-proximity layers that don't need an edge segment.
    pub fn query_point<'a>(&'a self, p: Point, radius_m: f32) -> impl Iterator<Item = u32> + 'a {
        let bbox = Aabb {
            min_x: p.x - radius_m,
            min_y: p.y - radius_m,
            max_x: p.x + radius_m,
            max_y: p.y + radius_m,
        };
        self.query_aabb(bbox)
    }
}

/// Decoded view into one feature's attribute blob. Lazy: only
/// reads the bytes for the field the caller asks about.
pub struct AttrView<'a> {
    schema: &'a AttrSchema,
    blob: &'a [u8],
}

impl<'a> AttrView<'a> {
    fn field(&self, name: &str) -> Option<&AttrField> {
        self.schema.fields.iter().find(|f| f.name == name)
    }

    pub fn f32(&self, name: &str) -> Option<f32> {
        let f = self.field(name)?;
        if f.ty != AttrType::F32 {
            return None;
        }
        let off = f.offset as usize;
        if off + 4 > self.blob.len() {
            return None;
        }
        Some(f32::from_le_bytes([
            self.blob[off],
            self.blob[off + 1],
            self.blob[off + 2],
            self.blob[off + 3],
        ]))
    }

    pub fn u32(&self, name: &str) -> Option<u32> {
        let f = self.field(name)?;
        if f.ty != AttrType::U32 {
            return None;
        }
        let off = f.offset as usize;
        if off + 4 > self.blob.len() {
            return None;
        }
        Some(u32::from_le_bytes([
            self.blob[off],
            self.blob[off + 1],
            self.blob[off + 2],
            self.blob[off + 3],
        ]))
    }

    pub fn u8(&self, name: &str) -> Option<u8> {
        let f = self.field(name)?;
        if f.ty != AttrType::U8 {
            return None;
        }
        let off = f.offset as usize;
        if off >= self.blob.len() {
            return None;
        }
        Some(self.blob[off])
    }
}

/// The on-disk vector store: one file, many named collections.
///
/// Cheap to drop — every [`VectorCollection`] handed out by
/// `collection()` keeps its own `Arc<Mmap>`, so the file remains
/// mapped as long as any collection is alive.
pub struct VectorStore {
    collections: HashMap<String, Arc<VectorCollection>>,
}

impl VectorStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, VectorError> {
        let file = File::open(path.as_ref())?;
        let mmap = unsafe { Mmap::map(&file)? };
        let mmap = Arc::new(mmap);
        if mmap.len() < HEADER_BYTES + 32 {
            return Err(VectorError::Malformed("file shorter than header+tail"));
        }
        let mut cursor = Cursor::new(&mmap[..]);
        let header = read_header(&mut cursor)?;
        check_header(&header, ArtifactKind::Vectors, VECTOR_FORMAT_VERSION)?;
        let _collection_count = cursor.read_u32::<LittleEndian>()?;
        let manifest_offset = cursor.read_u64::<LittleEndian>()?;
        let manifest_len = cursor.read_u64::<LittleEndian>()?;
        let mut _r = [0u8; 12];
        std::io::Read::read_exact(&mut cursor, &mut _r)?;

        if (manifest_offset + manifest_len) as usize > mmap.len() {
            return Err(VectorError::Malformed("manifest out of bounds"));
        }
        let manifest_bytes =
            &mmap[manifest_offset as usize..(manifest_offset + manifest_len) as usize];
        let manifest: Manifest = serde_json::from_slice(manifest_bytes)?;

        // Each collection's blob is owned by the mmap. We hand each
        // collection a clone of `Arc<Mmap>` so the mapping lives as
        // long as any collection is referenced — independent of
        // whether the parent `VectorStore` is dropped.
        let mut collections: HashMap<String, Arc<VectorCollection>> = HashMap::new();
        for cm in &manifest.collections {
            let blob_start = cm.offset as usize;
            let blob_end = blob_start + cm.byte_len as usize;
            if blob_end > mmap.len() {
                return Err(VectorError::Malformed("collection blob out of bounds"));
            }
            let blob = &mmap[blob_start..blob_end];
            let coll = parse_collection_blob(cm, blob, Arc::clone(&mmap))?;
            collections.insert(cm.name.clone(), Arc::new(coll));
        }

        Ok(Self { collections })
    }

    pub fn collection(&self, name: &str) -> Result<Arc<VectorCollection>, VectorError> {
        self.collections
            .get(name)
            .cloned()
            .ok_or_else(|| VectorError::UnknownCollection(name.to_string()))
    }

    pub fn try_collection(&self, name: &str) -> Option<Arc<VectorCollection>> {
        self.collections.get(name).cloned()
    }

    pub fn collection_names(&self) -> Vec<&str> {
        self.collections.keys().map(|s| s.as_str()).collect()
    }
}

fn parse_collection_blob(
    cm: &CollectionManifest,
    blob: &[u8],
    mmap: Arc<Mmap>,
) -> Result<VectorCollection, VectorError> {
    if blob.len() < 12 {
        return Err(VectorError::Malformed("collection blob too small"));
    }
    let feature_count = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]);
    let coord_count = u32::from_le_bytes([blob[4], blob[5], blob[6], blob[7]]);
    let attrs_bytes = u32::from_le_bytes([blob[8], blob[9], blob[10], blob[11]]);
    if feature_count != cm.feature_count {
        return Err(VectorError::Malformed(
            "feature_count mismatch in blob header",
        ));
    }

    let off_features = 12usize;
    let off_coords = off_features + (feature_count as usize) * FEATURE_INDEX_BYTES;
    let off_attrs = off_coords + (coord_count as usize) * std::mem::size_of::<Point>();
    let off_rtree = off_attrs + attrs_bytes as usize;
    if blob.len() < off_rtree {
        return Err(VectorError::Malformed(
            "collection blob shorter than declared sections",
        ));
    }

    let features_slice: &[FeatureIndex] = bytemuck::cast_slice(
        &blob[off_features..off_features + (feature_count as usize) * FEATURE_INDEX_BYTES],
    );
    let coords_slice: &[Point] = bytemuck::cast_slice(
        &blob[off_coords..off_coords + (coord_count as usize) * std::mem::size_of::<Point>()],
    );
    let attrs_slice = &blob[off_attrs..off_attrs + attrs_bytes as usize];

    // Rebuild the rstar from the per-feature AABBs already in the
    // FeatureIndex array. bulk_load is O(N log N) and routinely under
    // 100 ms even at 10⁶ features — cheaper than reading and trusting
    // a serialised tree, and removes the "deserialised tree contains
    // a stale interior pointer" failure mode entirely.
    let indexed: Vec<IndexedFeature> = features_slice
        .iter()
        .enumerate()
        .map(|(i, f)| IndexedFeature {
            aabb_min: [f.min_x, f.min_y],
            aabb_max: [f.max_x, f.max_y],
            feature_id: i as u32,
        })
        .collect();
    let rtree = RTree::bulk_load(indexed);

    // SAFETY: lifetimes constrained by the owning VectorStore's
    // `_mmap` field. The collection is stored in the same struct as
    // the mmap, so the 'static transmute is sound.
    let features_static =
        unsafe { std::mem::transmute::<&[FeatureIndex], &'static [FeatureIndex]>(features_slice) };
    let coords_static = unsafe { std::mem::transmute::<&[Point], &'static [Point]>(coords_slice) };
    let attrs_static = unsafe { std::mem::transmute::<&[u8], &'static [u8]>(attrs_slice) };

    Ok(VectorCollection {
        name: cm.name.clone(),
        kind: cm.kind,
        attr_schema: cm.attr_schema.clone(),
        features: features_static,
        coords: coords_static,
        attrs: attrs_static,
        rtree,
        ring_index: std::sync::Mutex::new(std::collections::HashMap::new()),
        _mmap: mmap,
    })
}

// ============================================================================
// Builder API
// ============================================================================

/// Build-time accumulator for one collection. Produced from a
/// streaming source (PG query, GeoJSON file, …) and finally folded
/// into a `VectorStoreWriter` which writes the file.
pub struct CollectionBuilder {
    pub name: String,
    pub kind: GeomKind,
    pub attr_schema: AttrSchema,
    features: Vec<FeatureIndex>,
    coords: Vec<Point>,
    attrs: Vec<u8>,
}

impl CollectionBuilder {
    pub fn new(name: impl Into<String>, kind: GeomKind, attr_schema: AttrSchema) -> Self {
        Self {
            name: name.into(),
            kind,
            attr_schema,
            features: Vec::new(),
            coords: Vec::new(),
            attrs: Vec::new(),
        }
    }

    /// Add one feature. `coords` is the polygon ring / linestring
    /// vertices / single point; `attrs` is a blob whose length must
    /// equal `attr_schema.bytes_per_feature`.
    pub fn push_feature(&mut self, coords: &[Point], attrs: &[u8]) -> Result<(), VectorError> {
        if attrs.len() != self.attr_schema.bytes_per_feature as usize {
            return Err(VectorError::Malformed(
                "attr blob length != schema bytes_per_feature",
            ));
        }
        let coord_offset = self.coords.len() as u32;
        let coord_count = coords.len() as u32;
        let attr_offset = self.attrs.len() as u32;
        let bbox = Aabb::of(coords);
        self.features.push(FeatureIndex {
            coord_offset,
            coord_count,
            attr_offset,
            _pad: 0,
            min_x: bbox.min_x,
            min_y: bbox.min_y,
            max_x: bbox.max_x,
            max_y: bbox.max_y,
        });
        self.coords.extend_from_slice(coords);
        self.attrs.extend_from_slice(attrs);
        Ok(())
    }

    pub fn feature_count(&self) -> u32 {
        self.features.len() as u32
    }
}

/// Streams a complete `norway.vectors` artifact in one pass.
pub struct VectorStoreWriter;

impl VectorStoreWriter {
    /// Serialise `collections` into the given writer. Returns the
    /// total bytes written. Atomic-rename / temp-file dance is the
    /// caller's job — this only writes contiguous bytes.
    pub fn write<W: Write>(
        mut sink: W,
        collections: Vec<CollectionBuilder>,
        timestamp_unix_sec: i64,
    ) -> Result<u64, VectorError> {
        // Serialise each collection blob into memory so we know
        // its size before writing the manifest. Vector data is on
        // the order of 10–500 MB total; keeping the per-collection
        // blob in RAM during build is comfortable.
        let mut blobs: Vec<Vec<u8>> = Vec::with_capacity(collections.len());
        for cb in &collections {
            let blob = serialise_collection_blob(cb)?;
            blobs.push(blob);
        }

        // Manifest is written after all the blobs so it can point
        // back at them. Layout:
        //   header (32) | tail (32) | blobs ... | manifest (JSON)
        let header_bytes = HEADER_BYTES as u64;
        let tail_bytes = 32u64; // collection_count + offsets + reserved
        let mut blob_offsets: Vec<u64> = Vec::with_capacity(blobs.len());
        let mut cursor = header_bytes + tail_bytes;
        for blob in &blobs {
            blob_offsets.push(cursor);
            cursor += blob.len() as u64;
        }
        let manifest_offset = cursor;
        let manifest = Manifest {
            collections: collections
                .iter()
                .enumerate()
                .map(|(i, cb)| CollectionManifest {
                    name: cb.name.clone(),
                    kind: cb.kind,
                    feature_count: cb.feature_count(),
                    attr_schema: cb.attr_schema.clone(),
                    offset: blob_offsets[i],
                    byte_len: blobs[i].len() as u64,
                })
                .collect(),
            build_timestamp_unix_sec: timestamp_unix_sec,
        };
        let manifest_bytes = serde_json::to_vec(&manifest)?;
        let manifest_len = manifest_bytes.len() as u64;

        write_header(
            &mut sink,
            &Header {
                kind: ArtifactKind::Vectors,
                format_version: VECTOR_FORMAT_VERSION,
                build_timestamp_unix_sec: timestamp_unix_sec,
            },
        )?;
        sink.write_u32::<LittleEndian>(collections.len() as u32)?;
        sink.write_u64::<LittleEndian>(manifest_offset)?;
        sink.write_u64::<LittleEndian>(manifest_len)?;
        sink.write_all(&[0u8; 12])?;
        for blob in &blobs {
            sink.write_all(blob)?;
        }
        sink.write_all(&manifest_bytes)?;
        sink.flush()?;
        Ok(manifest_offset + manifest_len)
    }
}

fn serialise_collection_blob(cb: &CollectionBuilder) -> Result<Vec<u8>, VectorError> {
    let mut buf: Vec<u8> = Vec::with_capacity(
        12 + cb.features.len() * FEATURE_INDEX_BYTES
            + cb.coords.len() * std::mem::size_of::<Point>()
            + cb.attrs.len()
            + 1024,
    );
    let feature_count = cb.features.len() as u32;
    let coord_count = cb.coords.len() as u32;
    let attrs_bytes = cb.attrs.len() as u32;
    buf.write_u32::<LittleEndian>(feature_count)?;
    buf.write_u32::<LittleEndian>(coord_count)?;
    buf.write_u32::<LittleEndian>(attrs_bytes)?;
    buf.extend_from_slice(bytemuck::cast_slice(&cb.features));
    buf.extend_from_slice(bytemuck::cast_slice(&cb.coords));
    buf.extend_from_slice(&cb.attrs);
    // No serialized rstar — the reader rebuilds it from the per-
    // feature AABBs at `open()` time. See `parse_collection_blob`.
    Ok(buf)
}

/// Convenience: write a finished store to a file with an
/// atomic-rename swap so a partial write is never visible.
pub fn write_store_to_path(
    path: &Path,
    collections: Vec<CollectionBuilder>,
    timestamp_unix_sec: i64,
) -> Result<u64, VectorError> {
    let tmp = path.with_extension("vectors.tmp");
    {
        let f = File::create(&tmp)?;
        let w = BufWriter::with_capacity(8 * 1024 * 1024, f);
        VectorStoreWriter::write(w, collections, timestamp_unix_sec)?;
    }
    std::fs::rename(&tmp, path)?;
    let len = std::fs::metadata(path)?.len();
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use turbo_tiles_geom::Point;

    fn build_square_water() -> CollectionBuilder {
        let schema = AttrSchema {
            fields: vec![AttrField {
                name: "area_m2".to_string(),
                ty: AttrType::F32,
                offset: 0,
            }],
            bytes_per_feature: 4,
        };
        let mut cb = CollectionBuilder::new("water", GeomKind::Polygon, schema);
        let ring = vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        let mut attrs = [0u8; 4];
        attrs.copy_from_slice(&100.0_f32.to_le_bytes());
        cb.push_feature(&ring, &attrs).unwrap();
        cb
    }

    #[test]
    fn write_then_open_round_trip() {
        let cb = build_square_water();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("norway.vectors");
        write_store_to_path(&path, vec![cb], 1_700_000_000).unwrap();
        let store = VectorStore::open(&path).unwrap();
        let coll = store.collection("water").unwrap();
        assert_eq!(coll.len(), 1);
        assert_eq!(coll.kind(), GeomKind::Polygon);
        let coords = coll.feature_coords(0);
        assert_eq!(coords.len(), 4);
        let attrs = coll.feature_attrs(0);
        assert!((attrs.f32("area_m2").unwrap() - 100.0).abs() < 1e-3);
    }

    #[test]
    fn query_aabb_finds_intersecting_feature() {
        let cb = build_square_water();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("norway.vectors");
        write_store_to_path(&path, vec![cb], 1_700_000_000).unwrap();
        let store = VectorStore::open(&path).unwrap();
        let coll = store.collection("water").unwrap();
        let hits: Vec<u32> = coll
            .query_aabb(Aabb {
                min_x: 5.0,
                min_y: 5.0,
                max_x: 6.0,
                max_y: 6.0,
            })
            .collect();
        assert_eq!(hits, vec![0]);
        let misses: Vec<u32> = coll
            .query_aabb(Aabb {
                min_x: 100.0,
                min_y: 100.0,
                max_x: 110.0,
                max_y: 110.0,
            })
            .collect();
        assert!(misses.is_empty());
    }

    #[test]
    fn unknown_collection_errors() {
        let cb = build_square_water();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("norway.vectors");
        write_store_to_path(&path, vec![cb], 1_700_000_000).unwrap();
        let store = VectorStore::open(&path).unwrap();
        assert!(matches!(
            store.collection("nonexistent"),
            Err(VectorError::UnknownCollection(_))
        ));
    }
}
