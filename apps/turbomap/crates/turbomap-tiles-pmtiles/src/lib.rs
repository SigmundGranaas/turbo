//! Read PMTiles v3 archives. A `.pmtiles` file packs millions of tiles
//! (raster or MVT) into a single static file with a Hilbert-ordered
//! directory. The same archive can be read three ways, all behind the
//! [`RangeReader`] trait:
//!
//! - [`PMTilesSource::open`] — a local file (offline bundle),
//! - [`PMTilesSource::open_bytes`] — bytes already in memory (e.g. a
//!   `include_bytes!` fixture),
//! - [`PMTilesSource::open_http`] — a static URL served with **HTTP range
//!   requests** (serverless online — S3/R2/any CDN, no tile server).
//!
//! ```no_run
//! use turbomap_tiles_pmtiles::PMTilesSource;
//! let local = PMTilesSource::open("norway.pmtiles").expect("open");
//! let cloud = PMTilesSource::open_http("https://cdn.example/planet.pmtiles").expect("open");
//! ```

mod directory;
mod header;
mod hilbert;
pub mod writer;

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use thiserror::Error;
use turbomap_core::{
    RasterFormat, RasterTile, TileError, TileId, TileSource, VectorTile, VectorTileSource,
};

pub use header::{Compression, TileType};

use crate::directory::{find_entry, parse_directory, DirEntry};
use crate::header::Header;

/// Random-access byte source under a PMTiles archive. One archive is a
/// handful of range reads — the 127-byte header, the root directory, any
/// leaf directories, and each tile's slice — so any backing store that can
/// return `len` bytes at an absolute `offset` works: a file, an in-memory
/// buffer, or an HTTP endpoint that honours `Range`.
pub trait RangeReader: Send + Sync {
    fn read_at(&self, offset: u64, len: usize) -> io::Result<Vec<u8>>;
}

/// Local-file backing. Seeks + reads under a mutex (the trait is shared).
struct FileRangeReader {
    file: Mutex<File>,
}

impl RangeReader for FileRangeReader {
    fn read_at(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        let mut f = self.file.lock();
        f.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; len];
        f.read_exact(&mut buf)?;
        Ok(buf)
    }
}

/// In-memory backing — the whole archive as bytes. Ideal for a bundled
/// `include_bytes!` fixture, and the reference backend tests compare HTTP
/// and file reads against (range reads are pure slices here).
pub struct BytesRangeReader {
    data: Arc<Vec<u8>>,
}

impl BytesRangeReader {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data: Arc::new(data) }
    }
}

impl RangeReader for BytesRangeReader {
    fn read_at(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        let start = offset as usize;
        let end = start
            .checked_add(len)
            .filter(|&e| e <= self.data.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "range past end of archive"))?;
        Ok(self.data[start..end].to_vec())
    }
}

/// HTTP backing — a static `.pmtiles` URL read with `Range` requests. No
/// tile server: any object store / CDN that supports byte ranges serves
/// the whole planet from one file.
pub struct HttpRangeReader {
    client: reqwest::blocking::Client,
    url: String,
}

impl RangeReader for HttpRangeReader {
    fn read_at(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        if len == 0 {
            return Ok(Vec::new());
        }
        let end = offset + len as u64 - 1;
        let resp = self
            .client
            .get(&self.url)
            .header(reqwest::header::RANGE, format!("bytes={offset}-{end}"))
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(io::Error::other)?;
        let bytes = resp.bytes().map_err(io::Error::other)?;
        // A server may legitimately return the whole body for a range it
        // doesn't honour; guard so we never hand back a short/over-long read.
        if bytes.len() < len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "range response shorter than requested",
            ));
        }
        Ok(bytes[..len].to_vec())
    }
}

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
    reader: Box<dyn RangeReader>,
    header: Header,
    root: Vec<DirEntry>,
    /// Memoised leaf directories, keyed by `(offset, length)` in the leaf
    /// region. Most archives have only a handful so a `Vec` is fine.
    leaves: Mutex<Vec<(u64, u32, Vec<DirEntry>)>>,
}

impl PMTilesSource {
    /// Open an archive from a local file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PMTilesError> {
        let file = File::open(path)?;
        Self::from_reader(Box::new(FileRangeReader {
            file: Mutex::new(file),
        }))
    }

    /// Open an archive already held in memory (e.g. a bundled fixture).
    pub fn open_bytes(data: Vec<u8>) -> Result<Self, PMTilesError> {
        Self::from_reader(Box::new(BytesRangeReader::new(data)))
    }

    /// Open a static `.pmtiles` URL, read serverlessly via HTTP `Range`
    /// requests. The header + root directory are fetched once here.
    pub fn open_http(url: impl Into<String>) -> Result<Self, PMTilesError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("turbomap-pmtiles/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(io::Error::other)?;
        Self::from_reader(Box::new(HttpRangeReader {
            client,
            url: url.into(),
        }))
    }

    /// Open against any [`RangeReader`] backend.
    pub fn from_reader(reader: Box<dyn RangeReader>) -> Result<Self, PMTilesError> {
        let header_buf = reader.read_at(0, header::HEADER_LEN)?;
        let header = Header::parse(&header_buf)?;
        // Root directory.
        let root_bytes = reader.read_at(header.root_dir_offset, header.root_dir_length as usize)?;
        let root_bytes = maybe_decompress(&root_bytes, header.internal_compression)?;
        let root = parse_directory(&root_bytes)?;
        Ok(Self {
            reader,
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
        let buf = self
            .reader
            .read_at(self.header.leaf_dirs_offset + offset, length as usize)?;
        let bytes = maybe_decompress(&buf, self.header.internal_compression)?;
        let entries = parse_directory(&bytes)?;
        let mut leaves = self.leaves.lock();
        leaves.push((offset, length, entries.clone()));
        Ok(entries)
    }

    fn read_data(&self, offset: u64, length: u32) -> Result<Vec<u8>, PMTilesError> {
        Ok(self
            .reader
            .read_at(self.header.tile_data_offset + offset, length as usize)?)
    }
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

    /// Build a minimum valid PMTiles archive (one raster tile at z0/0/0) as
    /// raw bytes.
    fn build_minimal_bytes(tile_bytes: &[u8]) -> Vec<u8> {
        // Directory: one entry for tile_id 0, offset 0, length=tile_bytes.len(),
        let mut dir_bytes = Vec::new();
        varint(&mut dir_bytes, 1); // entry count
        varint(&mut dir_bytes, 0); // tile_id delta from 0
        varint(&mut dir_bytes, 1); // run_length
        varint(&mut dir_bytes, tile_bytes.len() as u64); // length
        varint(&mut dir_bytes, 1); // offset = raw + 1 ⇒ raw 0

        let root_dir_offset: u64 = 127;
        let root_dir_length: u64 = dir_bytes.len() as u64;
        let tile_data_offset: u64 = root_dir_offset + root_dir_length;

        let mut header = [0u8; header::HEADER_LEN];
        header[0..7].copy_from_slice(&header::MAGIC);
        header[7] = 3;
        header[8..16].copy_from_slice(&root_dir_offset.to_le_bytes());
        header[16..24].copy_from_slice(&root_dir_length.to_le_bytes());
        header[56..64].copy_from_slice(&tile_data_offset.to_le_bytes());
        header[64..72].copy_from_slice(&(tile_bytes.len() as u64).to_le_bytes());
        header[72..80].copy_from_slice(&1u64.to_le_bytes());
        header[80..88].copy_from_slice(&1u64.to_le_bytes());
        header[88..96].copy_from_slice(&1u64.to_le_bytes());
        header[96] = 1; // clustered
        header[97] = 0; // internal_compression: none
        header[98] = 0; // tile_compression: none
        header[99] = 2; // tile_type: png
        header[100] = 0; // min_zoom
        header[101] = 0; // max_zoom

        let mut out = Vec::new();
        out.extend_from_slice(&header);
        out.extend_from_slice(&dir_bytes);
        out.extend_from_slice(tile_bytes);
        out
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

    /// A stub `RangeReader` that records every range it serves, so we can
    /// prove lookups go through `read_at` (the HTTP path's only contract)
    /// and never touch a file. Backed by the same bytes as the file.
    struct CountingReader {
        data: Vec<u8>,
        reads: std::sync::atomic::AtomicUsize,
    }
    impl RangeReader for CountingReader {
        fn read_at(&self, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
            self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let s = offset as usize;
            Ok(self.data[s..s + len].to_vec())
        }
    }

    #[test]
    fn range_backends_return_identical_tiles() {
        // The same archive read three ways — file, in-memory, and a stub
        // range reader (the HTTP contract) — must yield byte-identical tiles.
        // This is what lets one `.pmtiles` serve offline (file) and online
        // (HTTP range) interchangeably.
        let dir = TempDir::new().unwrap();
        let payload = b"deterministic tile payload";
        let bytes = build_minimal_bytes(payload);
        let file_path = build_minimal_archive(&dir, payload);

        let from_file = PMTilesSource::open(&file_path).unwrap();
        let from_bytes = PMTilesSource::open_bytes(bytes.clone()).unwrap();
        let counting = std::sync::Arc::new(CountingReader {
            data: bytes,
            reads: std::sync::atomic::AtomicUsize::new(0),
        });
        let from_range = PMTilesSource::from_reader(Box::new(CountingReaderHandle(counting.clone())))
            .unwrap();

        let id = TileId::new(0, 0, 0);
        let a = <PMTilesSource as TileSource>::request(&from_file, id).unwrap();
        let b = <PMTilesSource as TileSource>::request(&from_bytes, id).unwrap();
        let c = <PMTilesSource as TileSource>::request(&from_range, id).unwrap();
        assert_eq!(a.bytes, payload);
        assert_eq!(a.bytes, b.bytes, "file vs in-memory differ");
        assert_eq!(a.bytes, c.bytes, "file vs range-reader differ");
        // The range backend served at least the header, root dir, and tile.
        assert!(
            counting.reads.load(std::sync::atomic::Ordering::SeqCst) >= 3,
            "tile lookup should go through ranged read_at"
        );
    }

    /// Newtype so the test can both hold an `Arc` (to read the counter) and
    /// hand a `Box<dyn RangeReader>` to the source.
    struct CountingReaderHandle(std::sync::Arc<CountingReader>);
    impl RangeReader for CountingReaderHandle {
        fn read_at(&self, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
            self.0.read_at(offset, len)
        }
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
