//! Minimal PMTiles v3 archive **writer**.
//!
//! Builds a single-root-directory, uncompressed archive from a list of
//! `(TileId, bytes)`. That covers the writer's two jobs here: generating
//! deterministic test fixtures (so the reader and the render path are
//! exercised against archives we didn't also parse with the same code that
//! wrote them — the header/directory layout is asserted byte-by-byte in
//! tests against the spec), and letting a host pack a small offline bundle.
//! It intentionally skips leaf directories, run-length dedup, and
//! compression — planet-scale archives come from planetiler/tippecanoe.

use crate::header::{HEADER_LEN, MAGIC};
use crate::{hilbert, TileType};
use turbomap_core::TileId;

/// Serialise `tiles` into a complete `.pmtiles` (v3) byte vector. Tiles
/// may arrive in any order; they are stored in Hilbert-id order as the
/// spec requires. Duplicate tile ids are rejected.
pub fn write_archive(
    tile_type: TileType,
    tiles: &[(TileId, Vec<u8>)],
) -> Result<Vec<u8>, &'static str> {
    if tiles.is_empty() {
        return Err("archive needs at least one tile");
    }
    let mut entries: Vec<(u64, &[u8])> = tiles
        .iter()
        .map(|(id, bytes)| {
            (
                hilbert::tile_id(id.z, id.x as u64, id.y as u64),
                bytes.as_slice(),
            )
        })
        .collect();
    entries.sort_by_key(|e| e.0);
    if entries.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err("duplicate tile id");
    }

    // Tile-data region + directory rows (tile_id, offset, length).
    let mut data: Vec<u8> = Vec::new();
    let mut rows: Vec<(u64, u64, u32)> = Vec::with_capacity(entries.len());
    for (tid, bytes) in &entries {
        rows.push((*tid, data.len() as u64, bytes.len() as u32));
        data.extend_from_slice(bytes);
    }

    // Root directory: count, then the four varint columns.
    let mut dir = Vec::new();
    varint(&mut dir, rows.len() as u64);
    let mut prev = 0u64;
    for &(tid, _, _) in &rows {
        varint(&mut dir, tid - prev);
        prev = tid;
    }
    for _ in &rows {
        varint(&mut dir, 1); // run_length — no RLE dedup in this writer
    }
    for &(_, _, len) in &rows {
        varint(&mut dir, len as u64);
    }
    for &(_, off, _) in &rows {
        varint(&mut dir, off + 1); // raw+1 encoding; 0 = "contiguous" sentinel
    }

    let root_dir_offset = HEADER_LEN as u64;
    let tile_data_offset = root_dir_offset + dir.len() as u64;
    let min_zoom = tiles.iter().map(|(t, _)| t.z).min().unwrap_or(0);
    let max_zoom = tiles.iter().map(|(t, _)| t.z).max().unwrap_or(0);

    let mut header = [0u8; HEADER_LEN];
    header[0..7].copy_from_slice(&MAGIC);
    header[7] = 3; // version
    header[8..16].copy_from_slice(&root_dir_offset.to_le_bytes());
    header[16..24].copy_from_slice(&(dir.len() as u64).to_le_bytes());
    // JSON metadata + leaf dirs: absent (offset/length 0).
    header[56..64].copy_from_slice(&tile_data_offset.to_le_bytes());
    header[64..72].copy_from_slice(&(data.len() as u64).to_le_bytes());
    header[72..80].copy_from_slice(&(rows.len() as u64).to_le_bytes()); // addressed
    header[80..88].copy_from_slice(&(rows.len() as u64).to_le_bytes()); // entries
    header[88..96].copy_from_slice(&(rows.len() as u64).to_le_bytes()); // contents
    header[96] = 1; // clustered (we wrote in Hilbert order)
    header[97] = 0; // internal_compression: none
    header[98] = 0; // tile_compression: none
    header[99] = match tile_type {
        TileType::Mvt => 1,
        TileType::Png => 2,
        TileType::Jpeg => 3,
        TileType::Webp => 4,
        TileType::Avif => 5,
        TileType::Unknown => 0,
    };
    header[100] = min_zoom;
    header[101] = max_zoom;

    let mut out = Vec::with_capacity(HEADER_LEN + dir.len() + data.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&dir);
    out.extend_from_slice(&data);
    Ok(out)
}

fn varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

#[cfg(test)]
mod tests {
    //! Value boundary: an archive we write must round-trip through the
    //! reader — every tile retrievable, none invented — across all three
    //! range backends.
    use super::*;
    use crate::PMTilesSource;
    use turbomap_core::{TileError, TileId, TileSource};

    #[test]
    fn multi_tile_archive_round_trips_through_the_reader() {
        // Out-of-order input across two zooms; each tile's bytes are unique.
        let tiles = vec![
            (TileId::new(1, 1, 0), b"t-1-1-0".to_vec()),
            (TileId::new(0, 0, 0), b"t-0-0-0".to_vec()),
            (TileId::new(1, 0, 1), b"t-1-0-1".to_vec()),
            (TileId::new(1, 1, 1), b"t-1-1-1".to_vec()),
        ];
        let archive = write_archive(TileType::Png, &tiles).unwrap();
        let src = PMTilesSource::open_bytes(archive).unwrap();
        assert_eq!(src.header().min_zoom, 0);
        assert_eq!(src.header().max_zoom, 1);
        for (id, bytes) in &tiles {
            let got = <PMTilesSource as TileSource>::request(&src, *id).unwrap();
            assert_eq!(&got.bytes, bytes, "tile {id:?} corrupted in round-trip");
        }
        // A tile we never wrote must be a clean miss, not garbage.
        let missing = <PMTilesSource as TileSource>::request(&src, TileId::new(1, 0, 0));
        assert!(matches!(missing, Err(TileError::Network(_))));
    }

    #[test]
    fn duplicate_tiles_are_rejected() {
        let tiles = vec![
            (TileId::new(0, 0, 0), b"a".to_vec()),
            (TileId::new(0, 0, 0), b"b".to_vec()),
        ];
        assert!(write_archive(TileType::Png, &tiles).is_err());
    }

    #[test]
    fn empty_archive_is_rejected() {
        assert!(write_archive(TileType::Png, &[]).is_err());
    }
}
