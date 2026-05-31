//! Read PMTiles v3 archives. A `.pmtiles` file packs millions of tiles
//! (raster or MVT) into a single static file with a Hilbert-ordered
//! directory; the archive can sit on disk or be served via HTTP range
//! requests. This crate currently supports the local-file path.
//!
//! Typical use:
//!
//! ```no_run
//! use turbomap_tiles_pmtiles::PMTilesSource;
//! let src = PMTilesSource::open("norway.pmtiles").expect("open");
//! // src now implements turbomap_core::TileSource (raster) or
//! // VectorTileSource (vector) depending on the archive's tile_type.
//! ```

mod directory;
mod header;
mod hilbert;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use parking_lot::Mutex;
use thiserror::Error;
use turbomap_core::{
    RasterFormat, RasterTile, TileError, TileId, TileSource, VectorTile, VectorTileSource,
};

pub use header::{Compression, TileType};

use crate::directory::{find_entry, parse_directory, DirEntry};
use crate::header::Header;

#[derive(Debug, Error)]
pub enum PMTilesError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("file header is shorter than 127 bytes")]
    ShortHeader,
    #[error("not a PMTiles file (bad magic)")]
    BadMagic,
    #[error("unsupported PMTiles version: {0} (only v3 supported)")]
    UnsupportedVersion(u8),
    #[error("unknown compression: {0}")]
    UnknownCompression(u8),
    #[error("corrupt archive: {0}")]
    Corrupt(&'static str),
    #[error("tile not found")]
    NotFound,
}

/// A PMTiles archive opened against a local file. Tile lookups read from
/// the file at the right offset; the root directory is parsed once at
/// open time and cached in memory. Leaf directories are loaded on demand
/// and cached.
pub struct PMTilesSource {
    inner: Mutex<Inner>,
    header: Header,
    root: Vec<DirEntry>,
    /// Memoised leaf directories, keyed by `(offset, length)` in the leaf
    /// region. Most archives have only a handful so a `Vec` is fine.
    leaves: Mutex<Vec<(u64, u32, Vec<DirEntry>)>>,
}

struct Inner {
    file: File,
}

impl PMTilesSource {
    /// Open an archive from a local file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PMTilesError> {
        let mut file = File::open(path)?;
        let mut header_buf = [0u8; header::HEADER_LEN];
        file.read_exact(&mut header_buf)?;
        let header = Header::parse(&header_buf)?;
        // Root directory.
        let root_bytes = read_range(&mut file, header.root_dir_offset, header.root_dir_length)?;
        let root_bytes = maybe_decompress(&root_bytes, header.internal_compression)?;
        let root = parse_directory(&root_bytes)?;
        Ok(Self {
            inner: Mutex::new(Inner { file }),
            header,
            root,
            leaves: Mutex::new(Vec::new()),
        })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn tile_type(&self) -> TileType {
        self.header.tile_type
    }

    /// Read the raw tile bytes for `tile`. The returned bytes are
    /// **already decompressed** if the archive used tile-level
    /// compression (e.g. gzip on MVT tiles).
    pub fn read_tile_bytes(&self, tile: TileId) -> Result<Vec<u8>, PMTilesError> {
        let id = hilbert::tile_id(tile.z, tile.x as u64, tile.y as u64);
        let entry = self.lookup(id)?;
        let bytes = self.read_data(entry.offset, entry.length)?;
        maybe_decompress(&bytes, self.header.tile_compression)
    }

    /// Resolve the directory entry for `tile_id`, following any leaf
    /// pointers we need to.
    fn lookup(&self, tile_id: u64) -> Result<DirEntry, PMTilesError> {
        let entry = find_entry(&self.root, tile_id).ok_or(PMTilesError::NotFound)?;
        if !entry.is_leaf() {
            return Ok(entry);
        }
        // Leaf-directory pointer. Fetch (or reuse cached) leaf entries
        // and search within them.
        let leaf_offset = entry.offset;
        let leaf_length = entry.length;
        let leaf_entries = self.load_leaf(leaf_offset, leaf_length)?;
        let inner = find_entry(&leaf_entries, tile_id).ok_or(PMTilesError::NotFound)?;
        if inner.is_leaf() {
            // Nested leaves are technically allowed by the spec but we
            // don't traverse them recursively here — return NotFound.
            return Err(PMTilesError::NotFound);
        }
        Ok(inner)
    }

    fn load_leaf(&self, offset: u64, length: u32) -> Result<Vec<DirEntry>, PMTilesError> {
        // Quick check of the cache without re-reading from disk.
        {
            let leaves = self.leaves.lock();
            if let Some((_, _, entries)) =
                leaves.iter().find(|(o, l, _)| *o == offset && *l == length)
            {
                return Ok(entries.clone());
            }
        }
        let mut f = self.inner.lock();
        f.file
            .seek(SeekFrom::Start(self.header.leaf_dirs_offset + offset))?;
        let mut buf = vec![0u8; length as usize];
        f.file.read_exact(&mut buf)?;
        drop(f);
        let bytes = maybe_decompress(&buf, self.header.internal_compression)?;
        let entries = parse_directory(&bytes)?;
        let mut leaves = self.leaves.lock();
        leaves.push((offset, length, entries.clone()));
        Ok(entries)
    }

    fn read_data(&self, offset: u64, length: u32) -> Result<Vec<u8>, PMTilesError> {
        let mut f = self.inner.lock();
        f.file
            .seek(SeekFrom::Start(self.header.tile_data_offset + offset))?;
        let mut buf = vec![0u8; length as usize];
        f.file.read_exact(&mut buf)?;
        Ok(buf)
    }
}

fn read_range(file: &mut File, offset: u64, length: u64) -> Result<Vec<u8>, PMTilesError> {
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; length as usize];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

fn maybe_decompress(bytes: &[u8], comp: Compression) -> Result<Vec<u8>, PMTilesError> {
    match comp {
        Compression::None => Ok(bytes.to_vec()),
        Compression::Gzip => {
            let mut decoder = flate2::read::GzDecoder::new(bytes);
            let mut out = Vec::new();
            decoder.read_to_end(&mut out)?;
            Ok(out)
        }
        Compression::Brotli | Compression::Zstd | Compression::Unknown => {
            // These are valid in the spec but rare in the wild; skip for
            // now and surface a clear error so the host can swap to a
            // recompressed archive.
            Err(PMTilesError::UnknownCompression(match comp {
                Compression::Brotli => 3,
                Compression::Zstd => 4,
                _ => 1,
            }))
        }
    }
}

// ---- TileSource implementations ----------------------------------------

impl TileSource for PMTilesSource {
    fn request(&self, tile: TileId) -> Result<RasterTile, TileError> {
        if tile.z < self.header.min_zoom || tile.z > self.header.max_zoom {
            return Err(TileError::ZoomOutOfRange(tile.z));
        }
        let bytes = self
            .read_tile_bytes(tile)
            .map_err(|e| TileError::Network(e.to_string()))?;
        let format = match self.header.tile_type {
            TileType::Png => RasterFormat::Png,
            TileType::Jpeg => RasterFormat::Jpeg,
            TileType::Webp => RasterFormat::Webp,
            _ => {
                return Err(TileError::Decode(format!(
                    "PMTiles archive holds {:?} tiles; not a raster source",
                    self.header.tile_type
                )));
            }
        };
        Ok(RasterTile { bytes, format })
    }

    fn min_zoom(&self) -> u8 {
        self.header.min_zoom
    }

    fn max_zoom(&self) -> u8 {
        self.header.max_zoom
    }

    fn raster_format(&self) -> RasterFormat {
        match self.header.tile_type {
            TileType::Jpeg => RasterFormat::Jpeg,
            TileType::Webp => RasterFormat::Webp,
            _ => RasterFormat::Png,
        }
    }
}

impl VectorTileSource for PMTilesSource {
    fn request(&self, tile: TileId) -> Result<VectorTile, TileError> {
        if tile.z < self.header.min_zoom || tile.z > self.header.max_zoom {
            return Err(TileError::ZoomOutOfRange(tile.z));
        }
        if !self.header.tile_type.is_vector() {
            return Err(TileError::Decode(format!(
                "PMTiles archive holds {:?} tiles; not a vector source",
                self.header.tile_type
            )));
        }
        let bytes = self
            .read_tile_bytes(tile)
            .map_err(|e| TileError::Network(e.to_string()))?;
        turbomap_mvt::decode(&bytes).map_err(|e| TileError::Decode(e.to_string()))
    }

    fn min_zoom(&self) -> u8 {
        self.header.min_zoom
    }

    fn max_zoom(&self) -> u8 {
        self.header.max_zoom
    }
}

#[cfg(test)]
mod tests {
    //! Integration test: hand-build a minimum-viable PMTiles archive in a
    //! `TempDir`, open it, and verify both header introspection and tile
    //! retrieval. The header + directory + Hilbert parsers each have
    //! their own unit tests; this one wires them together.
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn varint(out: &mut Vec<u8>, mut v: u64) {
        while v >= 0x80 {
            out.push((v as u8 & 0x7f) | 0x80);
            v >>= 7;
        }
        out.push(v as u8);
    }

    /// Build a minimum valid PMTiles archive with one raster tile at
    /// (z=0, x=0, y=0). Returns the file path.
    fn build_minimal_archive(dir: &TempDir, tile_bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.path().join("test.pmtiles");

        // Directory: one entry for tile_id 0, offset 0, length=tile_bytes.len(),
        // run_length 1.
        let mut dir_bytes = Vec::new();
        varint(&mut dir_bytes, 1); // entry count
        varint(&mut dir_bytes, 0); // tile_id delta from 0
        varint(&mut dir_bytes, 1); // run_length
        varint(&mut dir_bytes, tile_bytes.len() as u64); // length
        varint(&mut dir_bytes, 1); // offset = raw + 1 ⇒ raw 0

        // Layout: 127B header, then root directory, then tile data.
        let root_dir_offset: u64 = 127;
        let root_dir_length: u64 = dir_bytes.len() as u64;
        let tile_data_offset: u64 = root_dir_offset + root_dir_length;

        let mut header = [0u8; header::HEADER_LEN];
        header[0..7].copy_from_slice(&header::MAGIC);
        header[7] = 3;
        header[8..16].copy_from_slice(&root_dir_offset.to_le_bytes());
        header[16..24].copy_from_slice(&root_dir_length.to_le_bytes());
        // metadata + leaves empty
        header[56..64].copy_from_slice(&tile_data_offset.to_le_bytes());
        header[64..72].copy_from_slice(&(tile_bytes.len() as u64).to_le_bytes());
        header[72..80].copy_from_slice(&1u64.to_le_bytes());
        header[80..88].copy_from_slice(&1u64.to_le_bytes());
        header[88..96].copy_from_slice(&1u64.to_le_bytes());
        header[96] = 1; // clustered
        header[97] = 0; // internal_compression: none (test archive)
        header[98] = 0; // tile_compression: none
        header[99] = 2; // tile_type: png (raster)
        header[100] = 0; // min_zoom
        header[101] = 0; // max_zoom

        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(&header).unwrap();
        file.write_all(&dir_bytes).unwrap();
        file.write_all(tile_bytes).unwrap();
        file.sync_all().unwrap();
        path
    }

    #[test]
    fn open_round_trips_a_raster_tile() {
        let dir = TempDir::new().unwrap();
        let payload = b"PNG-ish bytes, not a real image";
        let path = build_minimal_archive(&dir, payload);
        let src = PMTilesSource::open(&path).unwrap();
        assert_eq!(src.tile_type(), TileType::Png);
        assert_eq!(src.header().min_zoom, 0);
        assert_eq!(src.header().max_zoom, 0);

        let tile = <PMTilesSource as TileSource>::request(&src, TileId::new(0, 0, 0)).unwrap();
        assert_eq!(tile.bytes, payload);
        assert_eq!(tile.format, RasterFormat::Png);
    }

    #[test]
    fn out_of_range_zoom_is_rejected_before_disk_io() {
        let dir = TempDir::new().unwrap();
        let path = build_minimal_archive(&dir, b"x");
        let src = PMTilesSource::open(&path).unwrap();
        let err = <PMTilesSource as TileSource>::request(&src, TileId::new(5, 0, 0)).unwrap_err();
        assert!(matches!(err, TileError::ZoomOutOfRange(5)));
    }

    #[test]
    fn opening_a_non_pmtiles_file_returns_bad_magic() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("garbage");
        std::fs::write(&p, [0u8; 200]).unwrap();
        match PMTilesSource::open(&p) {
            Err(PMTilesError::BadMagic) => {}
            Err(other) => panic!("expected BadMagic, got {other:?}"),
            Ok(_) => panic!("expected error, opened garbage successfully"),
        }
    }
}
