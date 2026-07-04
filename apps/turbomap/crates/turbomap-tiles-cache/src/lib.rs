//! The ONE bounded, LRU on-disk byte cache (plan slice B5.1).
//!
//! Every tile transport (HTTP raster, HTTP vector, future range readers)
//! persists raw fetched bytes through this cache so a re-launch — or a pan
//! back — is offline-fast. Without a bound that directory grows without
//! limit: a planet's worth of tiles will fill any disk. This cache keeps
//! the directory under a byte budget by evicting least-recently-used
//! entries: both reads and writes refresh an entry's recency (file mtime),
//! and a sweep deletes oldest-first until the total is back under budget.
//!
//! Eviction is best-effort and crash-safe: writes are atomic
//! (temp-then-rename) and any I/O error simply leaves the entry be —
//! nothing the caller sees changes; the worst case is a slightly-too-large
//! cache that the next sweep trims.
//!
//! Deliberately knows nothing about tiles, formats, or sources — keys are
//! relative paths, values are bytes. The provider-chain work (B5) keeps all
//! format knowledge in codecs; a cache that understood payloads would be a
//! second place formats leak. (This crate previously held an *unbounded*
//! `DiskCachedSource<S>` adapter that nothing used — superseded by this
//! bounded store, which `turbomap-tiles-http` re-exports for back-compat.)

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

/// A directory of cached byte blobs kept under `budget_bytes`. Cloning
/// shares the same directory and write-counter (clones cache the same
/// store), so a cloned tile source caches coherently.
#[derive(Debug, Clone)]
pub struct DiskCache {
    root: PathBuf,
    budget_bytes: u64,
    /// Sweep for over-budget every `sweep_interval` writes — walking the
    /// tree on *every* write would be O(n) per tile.
    sweep_interval: u32,
    writes: Arc<AtomicU32>,
}

struct Entry {
    path: PathBuf,
    mtime: SystemTime,
    size: u64,
}

impl DiskCache {
    /// A cache rooted at `root` holding at most `budget_bytes`. Swept every
    /// 64 writes by default.
    pub fn new(root: impl Into<PathBuf>, budget_bytes: u64) -> Self {
        Self {
            root: root.into(),
            budget_bytes,
            sweep_interval: 64,
            writes: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Override the sweep cadence (mainly for tests — sweep every write).
    pub fn with_sweep_interval(mut self, n: u32) -> Self {
        self.sweep_interval = n.max(1);
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn budget_bytes(&self) -> u64 {
        self.budget_bytes
    }

    /// Read a cached blob at `rel` (a relative path like `z/x/y`). On a hit
    /// the entry's recency is refreshed so it survives eviction. Returns
    /// `None` on any miss or I/O error.
    pub fn read(&self, rel: &Path) -> Option<Vec<u8>> {
        let p = self.root.join(rel);
        let bytes = std::fs::read(&p).ok()?;
        // Best-effort LRU touch: opening with write (no truncate) lets us
        // bump mtime to "now". Failure is harmless — the entry is just a
        // touch staler than ideal.
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&p) {
            let _ = f.set_modified(SystemTime::now());
        }
        Some(bytes)
    }

    /// Atomically write `bytes` at `rel`, then (every `sweep_interval`
    /// writes) enforce the budget. Surfaces only write errors; eviction
    /// failures are swallowed.
    pub fn write(&self, rel: &Path, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        let p = self.root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = p.with_extension("tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &p)?;

        let n = self.writes.fetch_add(1, Ordering::Relaxed) + 1;
        if n.is_multiple_of(self.sweep_interval) {
            self.enforce_budget();
        }
        Ok(())
    }

    /// Current total size of cached blobs in bytes (walks the tree).
    pub fn total_bytes(&self) -> u64 {
        let mut entries = Vec::new();
        self.collect(&self.root, &mut entries);
        entries.iter().map(|e| e.size).sum()
    }

    /// Delete least-recently-used entries until the total is under budget.
    pub fn enforce_budget(&self) {
        let mut entries = Vec::new();
        self.collect(&self.root, &mut entries);
        let mut total: u64 = entries.iter().map(|e| e.size).sum();
        if total <= self.budget_bytes {
            return;
        }
        // Oldest mtime first — those are the least-recently used.
        entries.sort_by_key(|e| e.mtime);
        for e in entries {
            if total <= self.budget_bytes {
                break;
            }
            if std::fs::remove_file(&e.path).is_ok() {
                total = total.saturating_sub(e.size);
            }
        }
    }

    /// Recursively gather cache entries, skipping in-flight `.tmp` writes.
    fn collect(&self, dir: &Path, out: &mut Vec<Entry>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                self.collect(&path, out);
            } else if meta.is_file() {
                if path.extension().is_some_and(|e| e == "tmp") {
                    continue;
                }
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                out.push(Entry { path, mtime, size: meta.len() });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: a long-running session fetches far more tiles than
    //! fit in the budget; the cache must stay bounded and keep the tiles
    //! the user actually revisited, dropping the ones they didn't.
    use super::*;
    use std::path::Path;
    use std::thread::sleep;
    use std::time::Duration;
    use tempfile::TempDir;

    fn rel(z: u8, x: u32, y: u32) -> PathBuf {
        Path::new(&z.to_string()).join(x.to_string()).join(y.to_string())
    }

    #[test]
    fn stays_under_budget_evicting_oldest_first() {
        let dir = TempDir::new().unwrap();
        // Budget = 250 bytes; each tile is 100 bytes → at most 2 survive.
        let cache = DiskCache::new(dir.path(), 250).with_sweep_interval(1);
        let blob = vec![0u8; 100];
        for i in 0..5u32 {
            cache.write(&rel(14, i, 0), &blob).unwrap();
            sleep(Duration::from_millis(10)); // distinct mtimes
        }
        assert!(
            cache.total_bytes() <= 250,
            "cache must stay under budget, got {}",
            cache.total_bytes()
        );
        // The two most-recently-written tiles survive; the oldest are gone.
        assert!(cache.read(&rel(14, 4, 0)).is_some(), "newest kept");
        assert!(cache.read(&rel(14, 0, 0)).is_none(), "oldest evicted");
    }

    #[test]
    fn reading_an_entry_protects_it_from_eviction() {
        let dir = TempDir::new().unwrap();
        let cache = DiskCache::new(dir.path(), 250).with_sweep_interval(1);
        let blob = vec![0u8; 100];
        cache.write(&rel(0, 0, 0), &blob).unwrap();
        sleep(Duration::from_millis(10));
        cache.write(&rel(0, 1, 0), &blob).unwrap();
        sleep(Duration::from_millis(10));
        // Touch the oldest so it becomes most-recently-used.
        assert!(cache.read(&rel(0, 0, 0)).is_some());
        sleep(Duration::from_millis(10));
        // A third write triggers eviction; tile (1,0) is now the oldest.
        cache.write(&rel(0, 2, 0), &blob).unwrap();
        assert!(cache.read(&rel(0, 0, 0)).is_some(), "touched entry survives");
        assert!(cache.read(&rel(0, 1, 0)).is_none(), "untouched oldest evicted");
    }

    #[test]
    fn under_budget_keeps_everything() {
        let dir = TempDir::new().unwrap();
        let cache = DiskCache::new(dir.path(), 10_000).with_sweep_interval(1);
        for i in 0..5u32 {
            cache.write(&rel(14, i, 0), &[1u8; 100]).unwrap();
        }
        assert_eq!(cache.total_bytes(), 500);
        for i in 0..5u32 {
            assert!(cache.read(&rel(14, i, 0)).is_some());
        }
    }

    #[test]
    fn cache_persists_across_instances_sharing_the_same_root() {
        // The defining promise of a *disk* cache: a fresh instance over the
        // same directory serves bytes written by the previous one.
        let dir = TempDir::new().unwrap();
        {
            let cache = DiskCache::new(dir.path(), 10_000);
            cache.write(&rel(11, 1054, 706), b"z11x1054y706").unwrap();
        } // first instance dropped
        let cache2 = DiskCache::new(dir.path(), 10_000);
        assert_eq!(
            cache2.read(&rel(11, 1054, 706)).as_deref(),
            Some(b"z11x1054y706".as_slice())
        );
    }
}
