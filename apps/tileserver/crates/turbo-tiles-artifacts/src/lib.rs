//! Common artifact format primitives.
//!
//! Every artifact file written by `turbo-tiles-build` and read by a
//! primitive crate starts with a 32-byte fixed header. Per-artifact
//! payload follows.

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

/// "TURB" little-endian — sanity check that the file is one of ours.
pub const MAGIC: u32 = 0x4254_5552; // 'B' 'T' 'U' 'R' read little-endian

/// Bump the per-kind version when the on-disk layout for that kind
/// changes. Readers validate this against their expectation; mismatch
/// is a hard error (not silent fall-back to bad data).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum ArtifactKind {
    Dem = 1,
    Mask = 2,
    Graph = 3,
    Anchors = 4,
    Names = 5,
    /// Sibling of `Graph`: per-edge polyline geometry. Optional —
    /// when absent the route reconstructor falls back to straight
    /// segments between endpoint nodes (low-fidelity).
    GraphGeom = 6,
    /// Vector feature collections: water polygons, stream lines,
    /// wetland polygons, anchor points, etc. — anything cost layers
    /// reason about geometrically without paying the discretisation
    /// cost of a raster mask. Backs `turbo-tiles-vector`.
    Vectors = 7,
}

impl ArtifactKind {
    pub fn from_u32(v: u32) -> Option<Self> {
        Some(match v {
            1 => Self::Dem,
            2 => Self::Mask,
            3 => Self::Graph,
            4 => Self::Anchors,
            5 => Self::Names,
            6 => Self::GraphGeom,
            7 => Self::Vectors,
            _ => return None,
        })
    }

    /// Canonical on-disk filename under `${TILESERVER_ARTIFACT_DIR}/`.
    pub fn filename(self) -> &'static str {
        match self {
            Self::Dem => "norway.dem",
            Self::Mask => "norway.mask",
            Self::Graph => "norway.graph",
            Self::Anchors => "norway.anchors",
            Self::Names => "norway.names",
            Self::GraphGeom => "norway.graph_geom",
            Self::Vectors => "norway.vectors",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Header {
    pub kind: ArtifactKind,
    pub format_version: u32,
    pub build_timestamp_unix_sec: i64,
}

pub const HEADER_BYTES: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("bad magic: expected 0x{MAGIC:08X}, got 0x{0:08X}")]
    BadMagic(u32),
    #[error("unknown artifact kind: {0}")]
    UnknownKind(u32),
    #[error("artifact kind mismatch: expected {expected:?}, got {got:?}")]
    KindMismatch {
        expected: ArtifactKind,
        got: ArtifactKind,
    },
    #[error("format version mismatch for {kind:?}: file is v{file}, code expects v{code}")]
    VersionMismatch {
        kind: ArtifactKind,
        file: u32,
        code: u32,
    },
}

pub fn write_header<W: Write>(w: &mut W, h: &Header) -> Result<(), ArtifactError> {
    w.write_u32::<LittleEndian>(MAGIC)?;
    w.write_u32::<LittleEndian>(h.kind as u32)?;
    w.write_u32::<LittleEndian>(h.format_version)?;
    w.write_i64::<LittleEndian>(h.build_timestamp_unix_sec)?;
    // 12 reserved bytes — zero them out.
    w.write_all(&[0u8; 12])?;
    Ok(())
}

pub fn read_header<R: Read>(r: &mut R) -> Result<Header, ArtifactError> {
    let magic = r.read_u32::<LittleEndian>()?;
    if magic != MAGIC {
        return Err(ArtifactError::BadMagic(magic));
    }
    let kind_raw = r.read_u32::<LittleEndian>()?;
    let kind = ArtifactKind::from_u32(kind_raw).ok_or(ArtifactError::UnknownKind(kind_raw))?;
    let format_version = r.read_u32::<LittleEndian>()?;
    let build_timestamp_unix_sec = r.read_i64::<LittleEndian>()?;
    let mut _reserved = [0u8; 12];
    r.read_exact(&mut _reserved)?;
    Ok(Header {
        kind,
        format_version,
        build_timestamp_unix_sec,
    })
}

/// Validate a header's kind + version against the reader's expectations.
pub fn check_header(
    h: &Header,
    expected_kind: ArtifactKind,
    expected_version: u32,
) -> Result<(), ArtifactError> {
    if h.kind != expected_kind {
        return Err(ArtifactError::KindMismatch {
            expected: expected_kind,
            got: h.kind,
        });
    }
    if h.format_version != expected_version {
        return Err(ArtifactError::VersionMismatch {
            kind: h.kind,
            file: h.format_version,
            code: expected_version,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_header() {
        // Write + read must produce identical bytes; deterministic shape.
        let h = Header {
            kind: ArtifactKind::Dem,
            format_version: 1,
            build_timestamp_unix_sec: 1_700_000_000,
        };
        let mut buf = Vec::new();
        write_header(&mut buf, &h).unwrap();
        assert_eq!(buf.len(), HEADER_BYTES);
        let mut cursor = &buf[..];
        let parsed = read_header(&mut cursor).unwrap();
        assert_eq!(parsed.kind, ArtifactKind::Dem);
        assert_eq!(parsed.format_version, 1);
        assert_eq!(parsed.build_timestamp_unix_sec, 1_700_000_000);
    }

    #[test]
    fn rejects_bad_magic() {
        // Defence in depth: refusing a file with the wrong magic
        // catches "you pointed at the wrong file" early instead of
        // serving nonsense data.
        let mut buf = Vec::new();
        buf.extend_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        buf.extend_from_slice(&[0u8; HEADER_BYTES - 4]);
        let err = read_header(&mut &buf[..]).unwrap_err();
        assert!(matches!(err, ArtifactError::BadMagic(_)));
    }

    #[test]
    fn rejects_unknown_kind() {
        // A new kind that this binary doesn't know about — fail
        // explicitly rather than guess.
        let mut buf = Vec::new();
        buf.extend_from_slice(&MAGIC.to_le_bytes());
        buf.extend_from_slice(&99u32.to_le_bytes());
        buf.extend_from_slice(&[0u8; HEADER_BYTES - 8]);
        let err = read_header(&mut &buf[..]).unwrap_err();
        assert!(matches!(err, ArtifactError::UnknownKind(99)));
    }

    #[test]
    fn check_header_catches_kind_mismatch() {
        // A reader expecting a DEM that gets handed a graph file must
        // error — not silently sample the bytes as float elevations.
        let h = Header {
            kind: ArtifactKind::Graph,
            format_version: 1,
            build_timestamp_unix_sec: 0,
        };
        let err = check_header(&h, ArtifactKind::Dem, 1).unwrap_err();
        assert!(matches!(err, ArtifactError::KindMismatch { .. }));
    }

    #[test]
    fn check_header_catches_version_mismatch() {
        // A v1 reader against a v2 file must refuse rather than
        // misinterpret a layout it doesn't know.
        let h = Header {
            kind: ArtifactKind::Dem,
            format_version: 2,
            build_timestamp_unix_sec: 0,
        };
        let err = check_header(&h, ArtifactKind::Dem, 1).unwrap_err();
        assert!(matches!(err, ArtifactError::VersionMismatch { .. }));
    }

    #[test]
    fn kind_filenames_are_stable() {
        // Lock the conventional on-disk filenames so the build CLI
        // and runtime loader agree without explicit threading.
        assert_eq!(ArtifactKind::Dem.filename(), "norway.dem");
        assert_eq!(ArtifactKind::Graph.filename(), "norway.graph");
    }
}
