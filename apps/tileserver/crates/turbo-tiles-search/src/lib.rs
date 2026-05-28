//! Anchor search primitive.
//!
//! Single-file artifact `norway.anchors`:
//! ```text
//! [32 B] artifact header (kind=Anchors, version=SEARCH_FORMAT_VERSION)
//! [16 B] AnchorsMeta:
//!          count: u32
//!          names_size: u64
//!          reserved: [u8; 4]
//! [count × 32 B] AnchorRecord (POD, see struct)
//! [names_size B] UTF-8 concatenated names blob.
//! ```
//!
//! On `open()`:
//!   - rstar R-tree bulk-loaded over (x,y,idx) (~100 ms for 500k).
//!   - Sorted vector of (lowercased name, idx) for substring queries.

use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::Arc;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use memmap2::Mmap;
use rstar::RTree;
use thiserror::Error;
use turbo_tiles_artifacts::{
    check_header, read_header, ArtifactError, ArtifactKind, HEADER_BYTES,
};

pub const SEARCH_FORMAT_VERSION: u32 = 1;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    Unknown = 0,
    Summit = 1,
    Cabin = 2,
    Viewpoint = 3,
    Trailhead = 4,
    Parking = 5,
    Waterfeature = 6,
    NamedPlace = 7,
}

impl AnchorKind {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Summit,
            2 => Self::Cabin,
            3 => Self::Viewpoint,
            4 => Self::Trailhead,
            5 => Self::Parking,
            6 => Self::Waterfeature,
            7 => Self::NamedPlace,
            _ => Self::Unknown,
        }
    }
    pub fn from_text(s: &str) -> Self {
        match s {
            "summit" => Self::Summit,
            "cabin" => Self::Cabin,
            "viewpoint" => Self::Viewpoint,
            "trailhead" => Self::Trailhead,
            "parking" => Self::Parking,
            "waterfeature" => Self::Waterfeature,
            "named_place" => Self::NamedPlace,
            _ => Self::Unknown,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, serde::Serialize)]
pub struct AnchorRecord {
    pub id: u64,
    pub kind: u32,
    pub x: f32,
    pub y: f32,
    pub elev_m: f32,
    pub name_off: u32,
    pub name_len: u32,
}
pub const ANCHOR_RECORD_BYTES: usize = std::mem::size_of::<AnchorRecord>();
const _: () = assert!(ANCHOR_RECORD_BYTES == 32, "AnchorRecord must be 32 bytes");

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct AnchorsMeta {
    pub count: u32,
    pub names_size: u64,
}
pub const ANCHORS_META_BYTES: usize = 16;

pub fn write_meta<W: Write>(w: &mut W, m: &AnchorsMeta) -> std::io::Result<()> {
    w.write_u32::<LittleEndian>(m.count)?;
    w.write_u64::<LittleEndian>(m.names_size)?;
    w.write_all(&[0u8; 4])?;
    Ok(())
}
pub fn read_meta<R: Read>(r: &mut R) -> std::io::Result<AnchorsMeta> {
    let count = r.read_u32::<LittleEndian>()?;
    let names_size = r.read_u64::<LittleEndian>()?;
    let mut _r = [0u8; 4];
    r.read_exact(&mut _r)?;
    Ok(AnchorsMeta { count, names_size })
}

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("artifact: {0}")]
    Artifact(#[from] ArtifactError),
    #[error("malformed anchors: {0}")]
    Malformed(&'static str),
    #[error("utf-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TreeNode {
    pos: [f32; 2],
    idx: u32,
}
impl rstar::Point for TreeNode {
    type Scalar = f32;
    const DIMENSIONS: usize = 2;
    fn generate(mut g: impl FnMut(usize) -> Self::Scalar) -> Self {
        TreeNode {
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct AnchorHit {
    pub id: u64,
    pub kind: AnchorKind,
    pub name: Option<String>,
    pub x: f32,
    pub y: f32,
    pub elev_m: f32,
    pub distance_m: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexStats {
    pub meta: AnchorsMeta,
    pub file_size_bytes: u64,
    pub by_kind: serde_json::Map<String, serde_json::Value>,
}

pub struct Index {
    _mmap: Mmap,
    file_size_bytes: u64,
    records: &'static [AnchorRecord],
    names_blob: &'static [u8],
    rtree: RTree<TreeNode>,
    sorted_names: Arc<Vec<(String, u32)>>,
}

impl Index {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SearchError> {
        let file = File::open(path.as_ref())?;
        let file_size_bytes = file.metadata()?.len();
        let mmap = unsafe { Mmap::map(&file)? };
        if mmap.len() < HEADER_BYTES + ANCHORS_META_BYTES {
            return Err(SearchError::Malformed("file shorter than header+meta"));
        }
        let mut cur = Cursor::new(&mmap[..]);
        let header = read_header(&mut cur)?;
        check_header(&header, ArtifactKind::Anchors, SEARCH_FORMAT_VERSION)?;
        let meta = read_meta(&mut cur)?;

        let off_records = HEADER_BYTES + ANCHORS_META_BYTES;
        let recs_bytes = meta.count as usize * ANCHOR_RECORD_BYTES;
        let off_names = off_records + recs_bytes;
        let end = off_names + meta.names_size as usize;
        if mmap.len() < end {
            return Err(SearchError::Malformed("file shorter than declared sections"));
        }

        let bytes: &[u8] = &mmap;
        let records: &[AnchorRecord] =
            bytemuck::cast_slice(&bytes[off_records..off_records + recs_bytes]);
        let names_blob = &bytes[off_names..end];

        let records_static =
            unsafe { std::mem::transmute::<&[AnchorRecord], &'static [AnchorRecord]>(records) };
        let names_static =
            unsafe { std::mem::transmute::<&[u8], &'static [u8]>(names_blob) };

        let tree_nodes: Vec<TreeNode> = records_static
            .iter()
            .enumerate()
            .map(|(i, r)| TreeNode {
                pos: [r.x, r.y],
                idx: i as u32,
            })
            .collect();
        let rtree = RTree::bulk_load(tree_nodes);

        let mut sn: Vec<(String, u32)> = Vec::with_capacity(records_static.len());
        for (i, r) in records_static.iter().enumerate() {
            if r.name_len == 0 {
                continue;
            }
            let s = std::str::from_utf8(
                &names_static[r.name_off as usize..(r.name_off + r.name_len) as usize],
            )?;
            sn.push((s.to_lowercase(), i as u32));
        }
        sn.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(Self {
            _mmap: mmap,
            file_size_bytes,
            records: records_static,
            names_blob: names_static,
            rtree,
            sorted_names: Arc::new(sn),
        })
    }

    pub fn meta(&self) -> AnchorsMeta {
        AnchorsMeta {
            count: self.records.len() as u32,
            names_size: self.names_blob.len() as u64,
        }
    }

    pub fn stats(&self) -> IndexStats {
        let mut counts: std::collections::HashMap<&'static str, u64> =
            std::collections::HashMap::new();
        for r in self.records {
            let key = match AnchorKind::from_u32(r.kind) {
                AnchorKind::Unknown => "unknown",
                AnchorKind::Summit => "summit",
                AnchorKind::Cabin => "cabin",
                AnchorKind::Viewpoint => "viewpoint",
                AnchorKind::Trailhead => "trailhead",
                AnchorKind::Parking => "parking",
                AnchorKind::Waterfeature => "waterfeature",
                AnchorKind::NamedPlace => "named_place",
            };
            *counts.entry(key).or_default() += 1;
        }
        let mut by_kind = serde_json::Map::new();
        for (k, v) in counts {
            by_kind.insert(k.to_string(), serde_json::Value::from(v));
        }
        IndexStats {
            meta: self.meta(),
            file_size_bytes: self.file_size_bytes,
            by_kind,
        }
    }

    fn record_name(&self, r: &AnchorRecord) -> Option<String> {
        if r.name_len == 0 {
            return None;
        }
        let s = std::str::from_utf8(
            &self.names_blob[r.name_off as usize..(r.name_off + r.name_len) as usize],
        )
        .ok()?;
        Some(s.to_string())
    }

    fn make_hit(&self, idx: u32, query_x: f32, query_y: f32) -> AnchorHit {
        let r = &self.records[idx as usize];
        let dx = r.x - query_x;
        let dy = r.y - query_y;
        AnchorHit {
            id: r.id,
            kind: AnchorKind::from_u32(r.kind),
            name: self.record_name(r),
            x: r.x,
            y: r.y,
            elev_m: r.elev_m,
            distance_m: (dx * dx + dy * dy).sqrt(),
        }
    }

    pub fn nearest(
        &self,
        x: f32,
        y: f32,
        kind_filter: Option<AnchorKind>,
        n: usize,
    ) -> Vec<AnchorHit> {
        let mut out = Vec::with_capacity(n);
        for node in self.rtree.nearest_neighbor_iter(&TreeNode {
            pos: [x, y],
            idx: 0,
        }) {
            let r = &self.records[node.idx as usize];
            if let Some(kf) = kind_filter {
                if AnchorKind::from_u32(r.kind) != kf {
                    continue;
                }
            }
            out.push(self.make_hit(node.idx, x, y));
            if out.len() >= n {
                break;
            }
        }
        out
    }

    /// Anchors inside a bbox (EPSG:25833 m). Result is hard-capped
    /// at `max_count`; oversized queries stride-sample.
    pub fn anchors_in_bbox(
        &self,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        kind_filter: Option<AnchorKind>,
        max_count: usize,
    ) -> Vec<AnchorHit> {
        let lo = TreeNode {
            pos: [min_x as f32, min_y as f32],
            idx: u32::MAX,
        };
        let hi = TreeNode {
            pos: [max_x as f32, max_y as f32],
            idx: u32::MAX,
        };
        let aabb = rstar::AABB::from_corners(lo, hi);
        let mut matches: Vec<u32> = self
            .rtree
            .locate_in_envelope_intersecting(&aabb)
            .filter_map(|node| {
                let r = &self.records[node.idx as usize];
                if let Some(kf) = kind_filter {
                    if AnchorKind::from_u32(r.kind) != kf {
                        return None;
                    }
                }
                Some(node.idx)
            })
            .collect();
        if matches.len() > max_count {
            let stride = (matches.len() / max_count).max(1);
            matches = matches.into_iter().step_by(stride).take(max_count).collect();
        }
        matches
            .into_iter()
            .map(|idx| {
                let r = &self.records[idx as usize];
                self.make_hit(idx, r.x, r.y)
            })
            .collect()
    }

    pub fn search_name(&self, q: &str, limit: usize) -> Vec<AnchorHit> {
        let needle = q.to_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<(usize, u32)> = Vec::new();
        for (name, idx) in self.sorted_names.iter() {
            if name.contains(&needle) {
                hits.push((name.len(), *idx));
                if hits.len() >= limit * 4 {
                    break;
                }
            }
        }
        hits.sort_by_key(|(len, _)| *len);
        hits.truncate(limit);
        hits.into_iter()
            .map(|(_, idx)| {
                let r = &self.records[idx as usize];
                self.make_hit(idx, r.x, r.y)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_record_byte_size_locked() {
        assert_eq!(ANCHOR_RECORD_BYTES, 32);
    }

    #[test]
    fn meta_round_trip() {
        let m = AnchorsMeta {
            count: 42,
            names_size: 123,
        };
        let mut buf = Vec::new();
        write_meta(&mut buf, &m).unwrap();
        assert_eq!(buf.len(), ANCHORS_META_BYTES);
        let p = read_meta(&mut &buf[..]).unwrap();
        assert_eq!(p, m);
    }

    #[test]
    fn kind_text_round_trips() {
        for s in [
            "summit",
            "cabin",
            "viewpoint",
            "trailhead",
            "parking",
            "waterfeature",
            "named_place",
        ] {
            let k = AnchorKind::from_text(s);
            assert_ne!(k, AnchorKind::Unknown);
        }
    }
}
