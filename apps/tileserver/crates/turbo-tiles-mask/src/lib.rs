//! Refusal mask primitive.
//!
//! 2-bit-per-cell packed bitmap in EPSG:25833 at 100 m resolution.
//! Encodes whether a point is in water, glacier, or unrestricted.
//! Norway footprint: ~50 MB. Loaded as a whole into the mmap and
//! addressed directly; no LRU cache needed.

use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use memmap2::Mmap;
use thiserror::Error;
use turbo_tiles_artifacts::{
    check_header, read_header, ArtifactError, ArtifactKind, HEADER_BYTES,
};

pub const MASK_FORMAT_VERSION: u32 = 1;
pub const DEFAULT_RESOLUTION_M: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum RefusalKind {
    None = 0,
    Water = 1,
    Glacier = 2,
    /// Reserved for cliffs / restricted areas in a later stage.
    Reserved3 = 3,
}

impl RefusalKind {
    pub fn from_bits(b: u8) -> Self {
        match b & 0b11 {
            0 => Self::None,
            1 => Self::Water,
            2 => Self::Glacier,
            _ => Self::Reserved3,
        }
    }
    pub fn refused(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct MaskMeta {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub cells_x: u32,
    pub cells_y: u32,
    pub resolution_m: f32,
}

/// 4×f64 (32) + 2×u32 (8) + f32 (4) + 4 reserved = 48 bytes.
pub const MASK_META_BYTES: usize = 48;

pub fn write_meta<W: Write>(w: &mut W, m: &MaskMeta) -> std::io::Result<()> {
    w.write_f64::<LittleEndian>(m.min_x)?;
    w.write_f64::<LittleEndian>(m.min_y)?;
    w.write_f64::<LittleEndian>(m.max_x)?;
    w.write_f64::<LittleEndian>(m.max_y)?;
    w.write_u32::<LittleEndian>(m.cells_x)?;
    w.write_u32::<LittleEndian>(m.cells_y)?;
    w.write_f32::<LittleEndian>(m.resolution_m)?;
    w.write_all(&[0u8; 4])?;
    Ok(())
}

pub fn read_meta<R: Read>(r: &mut R) -> std::io::Result<MaskMeta> {
    let min_x = r.read_f64::<LittleEndian>()?;
    let min_y = r.read_f64::<LittleEndian>()?;
    let max_x = r.read_f64::<LittleEndian>()?;
    let max_y = r.read_f64::<LittleEndian>()?;
    let cells_x = r.read_u32::<LittleEndian>()?;
    let cells_y = r.read_u32::<LittleEndian>()?;
    let resolution_m = r.read_f32::<LittleEndian>()?;
    let mut _reserved = [0u8; 4];
    r.read_exact(&mut _reserved)?;
    Ok(MaskMeta {
        min_x,
        min_y,
        max_x,
        max_y,
        cells_x,
        cells_y,
        resolution_m,
    })
}

pub fn packed_bytes(cells: u64) -> u64 {
    (cells + 3) / 4
}

#[derive(Debug, Error)]
pub enum MaskError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("artifact: {0}")]
    Artifact(#[from] ArtifactError),
    #[error("malformed mask: {0}")]
    Malformed(&'static str),
    #[error("point out of coverage")]
    OutOfCoverage,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MaskCoverage {
    pub meta: MaskMeta,
    pub file_size_bytes: u64,
    pub cells_total: u64,
    pub cells_water: u64,
    pub cells_glacier: u64,
}

pub struct Mask {
    mmap: Mmap,
    meta: MaskMeta,
    file_size_bytes: u64,
    data_offset: u64,
}

impl Mask {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, MaskError> {
        let file = File::open(path.as_ref())?;
        let file_size_bytes = file.metadata()?.len();
        let mmap = unsafe { Mmap::map(&file)? };
        if mmap.len() < HEADER_BYTES + MASK_META_BYTES {
            return Err(MaskError::Malformed("file shorter than header+meta"));
        }
        let mut cursor = Cursor::new(&mmap[..]);
        let header = read_header(&mut cursor)?;
        check_header(&header, ArtifactKind::Mask, MASK_FORMAT_VERSION)?;
        let meta = read_meta(&mut cursor)?;
        let needed = packed_bytes(meta.cells_x as u64 * meta.cells_y as u64);
        let data_offset = (HEADER_BYTES + MASK_META_BYTES) as u64;
        if (mmap.len() as u64) - data_offset < needed {
            return Err(MaskError::Malformed("mask data shorter than expected"));
        }
        Ok(Self {
            mmap,
            meta,
            file_size_bytes,
            data_offset,
        })
    }

    pub fn meta(&self) -> &MaskMeta {
        &self.meta
    }

    /// Refusal class at an EPSG:25833 point.
    pub fn refused(&self, x: f64, y: f64) -> Result<RefusalKind, MaskError> {
        if x < self.meta.min_x
            || x > self.meta.max_x
            || y < self.meta.min_y
            || y > self.meta.max_y
        {
            return Err(MaskError::OutOfCoverage);
        }
        let res = self.meta.resolution_m as f64;
        let col = ((x - self.meta.min_x) / res).floor() as i64;
        let row = ((self.meta.max_y - y) / res).floor() as i64;
        let cx = self.meta.cells_x as i64;
        let cy = self.meta.cells_y as i64;
        if col < 0 || row < 0 || col >= cx || row >= cy {
            return Err(MaskError::OutOfCoverage);
        }
        let idx = row as u64 * self.meta.cells_x as u64 + col as u64;
        let byte_idx = (self.data_offset + idx / 4) as usize;
        let bit_off = (idx % 4) * 2;
        let b = self.mmap[byte_idx];
        Ok(RefusalKind::from_bits((b >> bit_off) & 0b11))
    }

    /// Iterate cells whose value is non-zero (i.e., this layer claims
    /// the cell) inside a world-space bbox. Returns (centre_x,
    /// centre_y, value). `max_count` caps the output and the iterator
    /// stride-samples when the bbox would otherwise exceed it.
    ///
    /// Used by the SPA's per-layer overlay to render the rasterised
    /// data as a translucent grid so curators can see exactly which
    /// cells the pathfinder treats as water / wetland / forest etc.
    pub fn cells_in_bbox(
        &self,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        max_count: usize,
    ) -> Vec<(f64, f64, u8)> {
        let res = self.meta.resolution_m as f64;
        // Clip the bbox to the mask extent.
        let qx0 = min_x.max(self.meta.min_x);
        let qx1 = max_x.min(self.meta.max_x);
        let qy0 = min_y.max(self.meta.min_y);
        let qy1 = max_y.min(self.meta.max_y);
        if qx0 >= qx1 || qy0 >= qy1 {
            return Vec::new();
        }
        let col0 = ((qx0 - self.meta.min_x) / res).floor() as i64;
        let col1 = ((qx1 - self.meta.min_x) / res).ceil() as i64;
        let row0 = ((self.meta.max_y - qy1) / res).floor() as i64;
        let row1 = ((self.meta.max_y - qy0) / res).ceil() as i64;
        let cx = self.meta.cells_x as i64;
        let cy = self.meta.cells_y as i64;
        let col0 = col0.max(0);
        let col1 = col1.min(cx);
        let row0 = row0.max(0);
        let row1 = row1.min(cy);
        let cells_in_window = ((col1 - col0).max(0) as usize)
            * ((row1 - row0).max(0) as usize);
        // Stride-sample when too many cells would be returned. A
        // stride of `s` keeps roughly `cells / s²` of them, evenly
        // distributed; visually still tells the curator where this
        // layer is dense even at country-wide zoom.
        let stride = if cells_in_window > max_count {
            ((cells_in_window as f64 / max_count as f64).sqrt().ceil() as i64).max(1)
        } else {
            1
        };
        let mut out = Vec::with_capacity(max_count.min(cells_in_window));
        let mut row = row0;
        while row < row1 {
            let mut col = col0;
            while col < col1 {
                let idx = row as u64 * self.meta.cells_x as u64 + col as u64;
                let byte_idx = (self.data_offset + idx / 4) as usize;
                let bit_off = (idx % 4) * 2;
                let v = (self.mmap[byte_idx] >> bit_off) & 0b11;
                if v != 0 {
                    let cx_m = self.meta.min_x + (col as f64 + 0.5) * res;
                    let cy_m = self.meta.max_y - (row as f64 + 0.5) * res;
                    out.push((cx_m, cy_m, v));
                    if out.len() >= max_count {
                        return out;
                    }
                }
                col += stride;
            }
            row += stride;
        }
        out
    }

    pub fn coverage(&self) -> MaskCoverage {
        let total = self.meta.cells_x as u64 * self.meta.cells_y as u64;
        let mut water = 0u64;
        let mut glacier = 0u64;
        let data = &self.mmap[self.data_offset as usize..];
        let full = (total / 4) as usize;
        let trailing = (total % 4) as usize;
        for &b in &data[..full] {
            for shift in 0..4 {
                match (b >> (shift * 2)) & 0b11 {
                    1 => water += 1,
                    2 => glacier += 1,
                    _ => {}
                }
            }
        }
        if trailing > 0 {
            let b = data[full];
            for shift in 0..trailing {
                match (b >> (shift * 2)) & 0b11 {
                    1 => water += 1,
                    2 => glacier += 1,
                    _ => {}
                }
            }
        }
        MaskCoverage {
            meta: self.meta,
            file_size_bytes: self.file_size_bytes,
            cells_total: total,
            cells_water: water,
            cells_glacier: glacier,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusal_kind_round_trip() {
        for v in 0u8..4 {
            assert_eq!(RefusalKind::from_bits(v) as u8, v);
        }
    }

    #[test]
    fn refused_predicate() {
        assert!(!RefusalKind::None.refused());
        assert!(RefusalKind::Water.refused());
        assert!(RefusalKind::Glacier.refused());
    }

    #[test]
    fn meta_round_trip() {
        let m = MaskMeta {
            min_x: 100_000.0,
            min_y: 6_500_000.0,
            max_x: 110_000.0,
            max_y: 6_510_000.0,
            cells_x: 100,
            cells_y: 100,
            resolution_m: 100.0,
        };
        let mut buf = Vec::new();
        write_meta(&mut buf, &m).unwrap();
        assert_eq!(buf.len(), MASK_META_BYTES);
        let parsed = read_meta(&mut &buf[..]).unwrap();
        assert_eq!(parsed, m);
    }

    #[test]
    fn packed_bytes_rounding() {
        assert_eq!(packed_bytes(0), 0);
        assert_eq!(packed_bytes(1), 1);
        assert_eq!(packed_bytes(4), 1);
        assert_eq!(packed_bytes(5), 2);
    }
}
