//! PMTiles v3 header parser. The header is the fixed first 127 bytes of
//! the archive (full spec at <https://github.com/protomaps/PMTiles>).

use crate::PMTilesError;

pub const HEADER_LEN: usize = 127;
pub const MAGIC: [u8; 7] = *b"PMTiles";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    Unknown,
    Gzip,
    Brotli,
    Zstd,
}

impl Compression {
    fn from_byte(b: u8) -> Result<Self, PMTilesError> {
        Ok(match b {
            0 => Self::None,
            1 => Self::Unknown,
            2 => Self::Gzip,
            3 => Self::Brotli,
            4 => Self::Zstd,
            other => return Err(PMTilesError::UnknownCompression(other)),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileType {
    Unknown,
    Mvt,
    Png,
    Jpeg,
    Webp,
    Avif,
}

impl TileType {
    fn from_byte(b: u8) -> Self {
        match b {
            1 => Self::Mvt,
            2 => Self::Png,
            3 => Self::Jpeg,
            4 => Self::Webp,
            5 => Self::Avif,
            _ => Self::Unknown,
        }
    }

    pub fn is_vector(self) -> bool {
        matches!(self, Self::Mvt)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub root_dir_offset: u64,
    pub root_dir_length: u64,
    pub json_metadata_offset: u64,
    pub json_metadata_length: u64,
    pub leaf_dirs_offset: u64,
    pub leaf_dirs_length: u64,
    pub tile_data_offset: u64,
    pub tile_data_length: u64,
    pub addressed_tiles_count: u64,
    pub tile_entries_count: u64,
    pub tile_contents_count: u64,
    pub clustered: bool,
    pub internal_compression: Compression,
    pub tile_compression: Compression,
    pub tile_type: TileType,
    pub min_zoom: u8,
    pub max_zoom: u8,
}

impl Header {
    pub fn parse(buf: &[u8]) -> Result<Self, PMTilesError> {
        if buf.len() < HEADER_LEN {
            return Err(PMTilesError::ShortHeader);
        }
        if buf[0..7] != MAGIC {
            return Err(PMTilesError::BadMagic);
        }
        if buf[7] != 3 {
            return Err(PMTilesError::UnsupportedVersion(buf[7]));
        }
        // 8..16: root_dir_offset (u64 LE), and so on. All fields use LE.
        fn u64_at(buf: &[u8], off: usize) -> u64 {
            u64::from_le_bytes(buf[off..off + 8].try_into().unwrap())
        }
        Ok(Self {
            root_dir_offset: u64_at(buf, 8),
            root_dir_length: u64_at(buf, 16),
            json_metadata_offset: u64_at(buf, 24),
            json_metadata_length: u64_at(buf, 32),
            leaf_dirs_offset: u64_at(buf, 40),
            leaf_dirs_length: u64_at(buf, 48),
            tile_data_offset: u64_at(buf, 56),
            tile_data_length: u64_at(buf, 64),
            addressed_tiles_count: u64_at(buf, 72),
            tile_entries_count: u64_at(buf, 80),
            tile_contents_count: u64_at(buf, 88),
            clustered: buf[96] != 0,
            internal_compression: Compression::from_byte(buf[97])?,
            tile_compression: Compression::from_byte(buf[98])?,
            tile_type: TileType::from_byte(buf[99]),
            min_zoom: buf[100],
            max_zoom: buf[101],
            // The remaining bounds + center fields aren't needed for tile
            // lookup; skipping them keeps the parser small.
        })
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: feeding the parser a hand-crafted 127-byte header
    //! returns matching fields. Real archive bytes are exercised by the
    //! source-level integration tests.
    use super::*;

    fn make_header() -> [u8; HEADER_LEN] {
        let mut h = [0u8; HEADER_LEN];
        h[0..7].copy_from_slice(&MAGIC);
        h[7] = 3; // version
        h[8..16].copy_from_slice(&100u64.to_le_bytes()); // root_dir_offset
        h[16..24].copy_from_slice(&50u64.to_le_bytes()); // root_dir_length
        h[24..32].copy_from_slice(&200u64.to_le_bytes()); // json_metadata_offset
        h[32..40].copy_from_slice(&10u64.to_le_bytes()); // json_metadata_length
        h[40..48].copy_from_slice(&300u64.to_le_bytes()); // leaf_dirs_offset
        h[48..56].copy_from_slice(&20u64.to_le_bytes()); // leaf_dirs_length
        h[56..64].copy_from_slice(&400u64.to_le_bytes()); // tile_data_offset
        h[64..72].copy_from_slice(&1000u64.to_le_bytes()); // tile_data_length
        h[72..80].copy_from_slice(&42u64.to_le_bytes()); // addressed_tiles_count
        h[80..88].copy_from_slice(&30u64.to_le_bytes()); // tile_entries_count
        h[88..96].copy_from_slice(&30u64.to_le_bytes()); // tile_contents_count
        h[96] = 1; // clustered
        h[97] = 2; // internal_compression: gzip
        h[98] = 2; // tile_compression: gzip
        h[99] = 1; // tile_type: mvt
        h[100] = 4; // min_zoom
        h[101] = 14; // max_zoom
        h
    }

    #[test]
    fn parses_a_well_formed_header() {
        let h = Header::parse(&make_header()).expect("parse");
        assert_eq!(h.root_dir_offset, 100);
        assert_eq!(h.tile_data_length, 1000);
        assert_eq!(h.internal_compression, Compression::Gzip);
        assert_eq!(h.tile_type, TileType::Mvt);
        assert!(h.tile_type.is_vector());
        assert_eq!(h.min_zoom, 4);
        assert_eq!(h.max_zoom, 14);
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut h = make_header();
        h[0] = b'X';
        assert!(matches!(Header::parse(&h), Err(PMTilesError::BadMagic)));
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut h = make_header();
        h[7] = 2; // v2
        assert!(matches!(
            Header::parse(&h),
            Err(PMTilesError::UnsupportedVersion(2))
        ));
    }

    #[test]
    fn rejects_short_buffer() {
        let h = vec![0u8; 50];
        assert!(matches!(Header::parse(&h), Err(PMTilesError::ShortHeader)));
    }
}
