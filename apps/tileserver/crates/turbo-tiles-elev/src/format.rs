//! DEM on-disk format **v2** — per-tile origins.
//!
//! v1 assumed every source raster aligned to a single global tile
//! grid. Real `raster2pgsql -t 256x256` data violates that: each
//! input GeoTIFF carries its own grid origin, so tiles between files
//! have different (ulx, uly) offsets and v1 rejected most of them.
//!
//! v2 stores **one tile per source raster** with its own
//! `(ulx, uly)`. The reader builds an rstar over the tile bboxes at
//! `open()` and answers a sample by finding the tile that contains
//! the query point. Tiles overlap only when source rasters did
//! (rare); a sample inside an overlap returns the latest source's
//! value (insertion order).
//!
//! Layout:
//!   [32 B] generic ArtifactHeader (kind=Dem, version=DEM_FORMAT_VERSION)
//!   [44 B] DemMeta:
//!            tile_count: u32
//!            tile_cells: u32           (256)
//!            pixel_size_m: f32         (10.0 for DTM10)
//!            nodata: f32
//!            compression: u8           (1 = zstd)
//!            reserved: [u8; 27]
//!   [tile_count × 32 B] TileEntry:
//!            ulx: f64
//!            uly: f64
//!            offset: u64
//!            compressed_size: u32
//!            reserved: [u8; 4]
//!   [...] Per-tile zstd-compressed payloads (`tile_cells² × f32`).
//!
//! Coordinate system: EPSG:25833 (UTM33N). All geometry inputs are in
//! meters under that projection; conversion from WGS84 happens at the
//! API boundary (see `crate::wgs84_to_utm33n`).
//!
//! Row-major across tiles, row-major within tiles. Y grows _south_ to
//! _north_, so `row = (max_y - y) / resolution` flips the axis to
//! match the file layout (north-up).

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Read, Write};

/// Bump on any layout change. v2 introduces per-tile origins to
/// accommodate real `raster2pgsql -t 256x256` outputs where each
/// input GeoTIFF starts at its own ulx/uly.
pub const DEM_FORMAT_VERSION: u32 = 2;

/// Tile size (cells per side). 256 keeps each decompressed tile at
/// 256 KiB of f32, which fits the 128 MB cache comfortably.
pub const DEFAULT_TILE_CELLS: u32 = 256;

/// `1` is the only supported compression today (zstd). Reserved for
/// switching codecs without bumping the whole format version.
pub const COMPRESSION_ZSTD: u8 = 1;

/// Sentinel elevation for cells outside coverage (matches DTM10 nodata).
pub const NODATA_SENTINEL: f32 = -9999.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DemMeta {
    pub tile_count: u32,
    pub tile_cells: u32,
    pub pixel_size_m: f32,
    pub nodata: f32,
    pub compression: u8,
}

/// 4 + 4 + 4 + 4 + 1 + 27 reserved = 44.
pub const DEM_META_BYTES: usize = 44;
const _DEM_META_BYTES_CHECK: () = {
    assert!(DEM_META_BYTES == 44, "DemMeta layout drifted; bump format version");
};

#[derive(Debug, Clone, Copy)]
pub struct TileEntry {
    /// Upperleft world coordinates in EPSG:25833 metres.
    /// `uly` is the maximum y of this tile (north edge); cells run
    /// southward from there.
    pub ulx: f64,
    pub uly: f64,
    /// Offset within the file (after header + meta + directory).
    pub offset: u64,
    /// Compressed bytes on disk.
    pub compressed_size: u32,
}

/// 8 + 8 + 8 + 4 + 4 reserved = 32.
pub const TILE_ENTRY_BYTES: usize = 32;

pub fn write_meta<W: Write>(w: &mut W, m: &DemMeta) -> io::Result<()> {
    w.write_u32::<LittleEndian>(m.tile_count)?;
    w.write_u32::<LittleEndian>(m.tile_cells)?;
    w.write_f32::<LittleEndian>(m.pixel_size_m)?;
    w.write_f32::<LittleEndian>(m.nodata)?;
    w.write_u8(m.compression)?;
    w.write_all(&[0u8; 27])?;
    Ok(())
}

pub fn read_meta<R: Read>(r: &mut R) -> io::Result<DemMeta> {
    let tile_count = r.read_u32::<LittleEndian>()?;
    let tile_cells = r.read_u32::<LittleEndian>()?;
    let pixel_size_m = r.read_f32::<LittleEndian>()?;
    let nodata = r.read_f32::<LittleEndian>()?;
    let compression = r.read_u8()?;
    let mut _reserved = [0u8; 27];
    r.read_exact(&mut _reserved)?;
    Ok(DemMeta {
        tile_count,
        tile_cells,
        pixel_size_m,
        nodata,
        compression,
    })
}

pub fn write_tile_entry<W: Write>(w: &mut W, e: &TileEntry) -> io::Result<()> {
    w.write_f64::<LittleEndian>(e.ulx)?;
    w.write_f64::<LittleEndian>(e.uly)?;
    w.write_u64::<LittleEndian>(e.offset)?;
    w.write_u32::<LittleEndian>(e.compressed_size)?;
    w.write_all(&[0u8; 4])?;
    Ok(())
}

pub fn read_tile_entry<R: Read>(r: &mut R) -> io::Result<TileEntry> {
    let ulx = r.read_f64::<LittleEndian>()?;
    let uly = r.read_f64::<LittleEndian>()?;
    let offset = r.read_u64::<LittleEndian>()?;
    let compressed_size = r.read_u32::<LittleEndian>()?;
    let mut _reserved = [0u8; 4];
    r.read_exact(&mut _reserved)?;
    Ok(TileEntry {
        ulx,
        uly,
        offset,
        compressed_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> DemMeta {
        DemMeta {
            tile_count: 16,
            tile_cells: DEFAULT_TILE_CELLS,
            pixel_size_m: 10.0,
            nodata: NODATA_SENTINEL,
            compression: COMPRESSION_ZSTD,
        }
    }

    #[test]
    fn meta_round_trip() {
        let m = sample_meta();
        let mut buf = Vec::new();
        write_meta(&mut buf, &m).unwrap();
        assert_eq!(buf.len(), DEM_META_BYTES);
        let parsed = read_meta(&mut &buf[..]).unwrap();
        assert_eq!(parsed, m);
    }

    #[test]
    fn tile_entry_round_trip() {
        let e = TileEntry {
            ulx: 123_456.0,
            uly: 6_543_210.0,
            offset: 0x1234_5678_9ABC,
            compressed_size: 12_345,
        };
        let mut buf = Vec::new();
        write_tile_entry(&mut buf, &e).unwrap();
        assert_eq!(buf.len(), TILE_ENTRY_BYTES);
        let parsed = read_tile_entry(&mut &buf[..]).unwrap();
        assert_eq!(parsed.ulx, e.ulx);
        assert_eq!(parsed.uly, e.uly);
        assert_eq!(parsed.offset, e.offset);
        assert_eq!(parsed.compressed_size, e.compressed_size);
    }
}
