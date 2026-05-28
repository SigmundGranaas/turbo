//! Bounded LRU tile cache for decompressed DEM tiles.
//!
//! A single global `Mutex` is fine: each `sample()` call holds the
//! lock only long enough to look up + bump LRU order, and the hot
//! path is a hash lookup (microseconds). The decompression step
//! happens *outside* the lock when a tile is missing.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

/// One-shot identifier for a tile within a DEM (`tile_col`, `tile_row`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId(pub u32, pub u32);

/// A decompressed tile: row-major f32 elevations.
pub type TilePayload = Arc<Vec<f32>>;

/// Eviction tracked by an integer access counter, not a linked list,
/// so we don't have to fight Rust's borrow checker on intrusive lists.
/// On insertion we record `tick`; on eviction we drop the lowest tick.
/// Sweep cost is O(N) but only when the cache is full, and N is
/// bounded (~512 entries with default sizing).
struct Entry {
    payload: TilePayload,
    last_used: u64,
    bytes: usize,
}

pub struct TileCache {
    map: Mutex<Inner>,
    capacity_bytes: usize,
}

struct Inner {
    entries: HashMap<TileId, Entry>,
    tick: u64,
    total_bytes: usize,
    // Lightweight stats — read via `TileCache::stats()` from
    // /v1/debug/elev/coverage. Cheap to maintain (one u64 inc per op).
    hits: u64,
    misses: u64,
    evictions: u64,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct CacheStats {
    pub entries: usize,
    pub total_bytes: usize,
    pub capacity_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

impl TileCache {
    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            map: Mutex::new(Inner {
                entries: HashMap::new(),
                tick: 0,
                total_bytes: 0,
                hits: 0,
                misses: 0,
                evictions: 0,
            }),
            capacity_bytes,
        }
    }

    /// Fetch a tile. Returns `Some` only if already cached — callers
    /// are expected to decompress + `insert` on a miss. Keeping the
    /// fetch + decompress flow explicit (rather than a closure) lets
    /// us release the lock during decompression and lets the caller
    /// surface decompression I/O errors directly.
    pub fn get(&self, id: TileId) -> Option<TilePayload> {
        let mut inner = self.map.lock();
        inner.tick = inner.tick.wrapping_add(1);
        let tick = inner.tick;
        if let Some(e) = inner.entries.get_mut(&id) {
            e.last_used = tick;
            let payload = e.payload.clone();
            inner.hits += 1;
            Some(payload)
        } else {
            inner.misses += 1;
            None
        }
    }

    pub fn insert(&self, id: TileId, payload: TilePayload) {
        let bytes = payload.len() * std::mem::size_of::<f32>();
        let mut inner = self.map.lock();
        inner.tick = inner.tick.wrapping_add(1);
        let tick = inner.tick;
        // Pre-emptive eviction: drop coldest entries until under cap.
        while inner.total_bytes + bytes > self.capacity_bytes && !inner.entries.is_empty() {
            let coldest = inner
                .entries
                .iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(k, _)| *k);
            if let Some(k) = coldest {
                if let Some(e) = inner.entries.remove(&k) {
                    inner.total_bytes -= e.bytes;
                    inner.evictions += 1;
                }
            } else {
                break;
            }
        }
        inner.total_bytes += bytes;
        inner.entries.insert(
            id,
            Entry {
                payload,
                last_used: tick,
                bytes,
            },
        );
    }

    pub fn stats(&self) -> CacheStats {
        let inner = self.map.lock();
        CacheStats {
            entries: inner.entries.len(),
            total_bytes: inner.total_bytes,
            capacity_bytes: self.capacity_bytes,
            hits: inner.hits,
            misses: inner.misses,
            evictions: inner.evictions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_then_get_hits() {
        let c = TileCache::new(10 * 1024 * 1024);
        let t = Arc::new(vec![1.0f32; 256 * 256]);
        c.insert(TileId(0, 0), t.clone());
        let got = c.get(TileId(0, 0)).expect("present");
        assert_eq!(got.len(), t.len());
        let s = c.stats();
        assert_eq!(s.entries, 1);
        assert_eq!(s.hits, 1);
    }

    #[test]
    fn miss_increments_miss_counter() {
        let c = TileCache::new(10 * 1024 * 1024);
        assert!(c.get(TileId(99, 99)).is_none());
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn evicts_when_over_capacity() {
        // 3 tiles × 1 KB = 3 KB; cap is 2 KB → first insert must evict.
        let c = TileCache::new(2 * 1024);
        let mk = |v: f32| Arc::new(vec![v; 256]); // 1 KB
        c.insert(TileId(0, 0), mk(1.0));
        c.insert(TileId(1, 0), mk(2.0));
        c.insert(TileId(2, 0), mk(3.0));
        let s = c.stats();
        // At least one eviction happened to make room.
        assert!(s.evictions >= 1);
        assert!(s.total_bytes <= c.capacity_bytes);
    }
}
