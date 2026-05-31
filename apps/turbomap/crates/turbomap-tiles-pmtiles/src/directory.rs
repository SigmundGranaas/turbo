//! PMTiles v3 directory format. Directories are arrays of entries sorted
//! by `tile_id`. Each directory is stored on disk as four varint-encoded
//! columns followed by the entries themselves:
//!
//!   entries_count, then for each entry:
//!     tile_id_delta (delta-encoded), run_length, offset, length
//!
//! `offset == 0` means "this entry is a leaf pointer" — follow it into
//! the leaf-dirs region to find a child directory; everything else
//! points into the tile-data region.

use crate::PMTilesError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirEntry {
    pub tile_id: u64,
    pub offset: u64,
    pub length: u32,
    pub run_length: u32,
}

impl DirEntry {
    /// `true` if this entry points to a leaf directory rather than tile
    /// bytes. Leaf-entry `offset` is into `leaf_dirs_offset`, not the tile
    /// data region.
    pub fn is_leaf(&self) -> bool {
        self.run_length == 0
    }
}

pub fn parse_directory(bytes: &[u8]) -> Result<Vec<DirEntry>, PMTilesError> {
    let mut cursor = Cursor::new(bytes);
    let n = cursor.read_varint()? as usize;
    let mut entries = Vec::with_capacity(n);

    // Each field is delta- or raw-varint encoded across the whole column.
    let mut last_tile_id: u64 = 0;
    let mut tile_ids = Vec::with_capacity(n);
    for _ in 0..n {
        let delta = cursor.read_varint()?;
        last_tile_id = last_tile_id.wrapping_add(delta);
        tile_ids.push(last_tile_id);
    }

    let mut run_lengths = Vec::with_capacity(n);
    for _ in 0..n {
        run_lengths.push(cursor.read_varint()? as u32);
    }

    let mut lengths = Vec::with_capacity(n);
    for _ in 0..n {
        lengths.push(cursor.read_varint()? as u32);
    }

    let mut offsets = Vec::with_capacity(n);
    let mut last_offset: u64 = 0;
    for i in 0..n {
        let raw = cursor.read_varint()?;
        let offset = if raw == 0 {
            // Sentinel: this entry's tile bytes are contiguous with the
            // previous entry's. PMTiles spec calls this "+1 means
            // append" — concretely: `last_offset + lengths[i-1]`.
            if i == 0 {
                0
            } else {
                last_offset + lengths[i - 1] as u64
            }
        } else {
            raw - 1
        };
        offsets.push(offset);
        last_offset = offset;
    }

    for i in 0..n {
        entries.push(DirEntry {
            tile_id: tile_ids[i],
            offset: offsets[i],
            length: lengths[i],
            run_length: run_lengths[i],
        });
    }
    Ok(entries)
}

/// Binary search through a directory for the entry covering `tile_id`.
/// PMTiles uses run-length encoding for contiguous tile IDs that share
/// the same content (e.g. ocean tiles), so the matching entry may have
/// `tile_id <= query < tile_id + run_length`.
pub fn find_entry(entries: &[DirEntry], query: u64) -> Option<DirEntry> {
    // Locate via partition_point — first entry whose tile_id > query.
    let idx = entries.partition_point(|e| e.tile_id <= query);
    if idx == 0 {
        return None;
    }
    let candidate = entries[idx - 1];
    if candidate.is_leaf() {
        // Leaf entries don't have a meaningful run_length — caller must
        // follow into the leaf directory.
        return Some(candidate);
    }
    if query < candidate.tile_id + candidate.run_length as u64 {
        Some(candidate)
    } else {
        None
    }
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn read_varint(&mut self) -> Result<u64, PMTilesError> {
        let mut result: u64 = 0;
        let mut shift = 0u32;
        loop {
            if self.pos >= self.buf.len() {
                return Err(PMTilesError::Corrupt("varint truncated"));
            }
            let b = self.buf[self.pos];
            self.pos += 1;
            result |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
            if shift > 63 {
                return Err(PMTilesError::Corrupt("varint too long"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: a developer building a PMTiles archive can call
    //! `parse_directory` on the bytes we produce in our writer, OR on
    //! bytes from a third-party producer (Felt, planetiler, …), and get
    //! the same entry set back.
    //!
    //! Helper: hand-build a directory with three entries.
    use super::*;

    fn varint_push(out: &mut Vec<u8>, mut v: u64) {
        while v >= 0x80 {
            out.push((v as u8 & 0x7f) | 0x80);
            v >>= 7;
        }
        out.push(v as u8);
    }

    fn make_directory(entries: &[(u64, u64, u32, u32)]) -> Vec<u8> {
        // entries: (tile_id, offset, length, run_length)
        let mut out = Vec::new();
        varint_push(&mut out, entries.len() as u64);
        // tile_id deltas
        let mut prev = 0u64;
        for &(tid, _, _, _) in entries {
            varint_push(&mut out, tid - prev);
            prev = tid;
        }
        // run lengths
        for &(_, _, _, rl) in entries {
            varint_push(&mut out, rl as u64);
        }
        // lengths
        for &(_, _, len, _) in entries {
            varint_push(&mut out, len as u64);
        }
        // offsets — write as raw+1 (no contiguous optimisation in this helper)
        for &(_, off, _, _) in entries {
            varint_push(&mut out, off + 1);
        }
        out
    }

    #[test]
    fn parses_a_minimal_three_entry_directory() {
        let bytes = make_directory(&[(10, 0, 100, 1), (12, 100, 200, 1), (20, 300, 50, 1)]);
        let entries = parse_directory(&bytes).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].tile_id, 10);
        assert_eq!(entries[0].offset, 0);
        assert_eq!(entries[0].length, 100);
        assert_eq!(entries[1].tile_id, 12);
        assert_eq!(entries[2].offset, 300);
    }

    #[test]
    fn find_entry_handles_run_length_encoded_ids() {
        // One entry that covers tile_ids 10..15 (run_length 5).
        let bytes = make_directory(&[(10, 0, 100, 5)]);
        let entries = parse_directory(&bytes).unwrap();
        assert!(find_entry(&entries, 10).is_some());
        assert!(find_entry(&entries, 14).is_some());
        assert!(find_entry(&entries, 15).is_none(), "outside run");
        assert!(find_entry(&entries, 9).is_none(), "before first entry");
    }

    #[test]
    fn find_entry_returns_leaf_pointers_regardless_of_run_length() {
        // A leaf entry has run_length 0 and points elsewhere.
        let bytes = make_directory(&[(0, 4096, 50, 0)]);
        let entries = parse_directory(&bytes).unwrap();
        let hit = find_entry(&entries, 100).expect("leaf covers all tile_ids ≥ its own");
        assert!(hit.is_leaf());
    }

    #[test]
    fn truncated_varint_is_a_corrupt_error() {
        // A single byte with the continuation bit set — incomplete.
        let bad = vec![0x80u8];
        assert!(matches!(
            parse_directory(&bad),
            Err(PMTilesError::Corrupt(_))
        ));
    }
}
