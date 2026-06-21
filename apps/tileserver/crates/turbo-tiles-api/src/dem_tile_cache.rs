//! Rendered DEM-tile cache + render throttle.
//!
//! `/v1/dem/rgb` re-renders each tile from scratch — 66k inverse-projections +
//! bilinear DEM samples + PNG encode — which is CPU-bound and (measured)
//! congestion-collapses past a handful of concurrent renders (p50 430ms @8 →
//! 9.5s @64). The DTM is immutable per deploy, so every rendered tile is
//! deterministic + reusable forever. This adds:
//!
//!   1. A two-tier WRITE-THROUGH cache — a small in-RAM byte-LRU in front of an
//!      SSD disk byte-LRU. Everything produced is cached; a hit serves bytes in
//!      ~1ms and is not CPU-bound, so warm DEM scales like a CDN.
//!   2. Byte-budgeted eviction on BOTH tiers — when the disk cache reaches its
//!      ceiling it deletes the least-recently-used tiles (an in-memory index
//!      keeps eviction O(log n), no directory scans).
//!   3. A render-concurrency limiter — a *miss* that can't get a permit returns
//!      429 (throttle) instead of piling onto the renderer and collapsing it.
//!      Hits never take a permit, so a warm server stays fully concurrent.
//!
//! All tunables come from env (see [`DemTileCache::from_env`]).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Identity of a rendered tile. `halo` is part of the key — a halo'd tile is a
/// different PNG than the bare tile.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TileKey {
    pub z: u8,
    pub x: u32,
    pub y: u32,
    pub halo: u32,
}

/// Generic byte-weighted LRU. `seq` is a monotonic access clock; `order` keeps
/// keys sorted by last-access so eviction pops the least-recently-used in
/// O(log n). `put` returns the keys it evicted so the disk tier can delete them.
struct ByteLru<V> {
    budget: u64,
    used: u64,
    clock: u64,
    map: HashMap<TileKey, Entry<V>>,
    order: BTreeMap<u64, TileKey>,
}

struct Entry<V> {
    val: V,
    size: u64,
    seq: u64,
}

impl<V: Clone> ByteLru<V> {
    fn new(budget: u64) -> Self {
        Self {
            budget,
            used: 0,
            clock: 0,
            map: HashMap::new(),
            order: BTreeMap::new(),
        }
    }

    fn get(&mut self, k: &TileKey) -> Option<V> {
        let e = self.map.get_mut(k)?;
        self.order.remove(&e.seq);
        self.clock += 1;
        e.seq = self.clock;
        self.order.insert(e.seq, *k);
        Some(e.val.clone())
    }

    /// Insert/replace, then evict LRU until within budget. Never evicts the
    /// just-inserted key (a single tile larger than the budget still serves).
    fn put(&mut self, k: TileKey, size: u64, val: V) -> Vec<TileKey> {
        if let Some(old) = self.map.remove(&k) {
            self.order.remove(&old.seq);
            self.used -= old.size;
        }
        self.clock += 1;
        self.map.insert(
            k,
            Entry {
                val,
                size,
                seq: self.clock,
            },
        );
        self.order.insert(self.clock, k);
        self.used += size;

        let mut evicted = Vec::new();
        while self.used > self.budget {
            let Some((&seq, &victim)) = self.order.iter().next() else {
                break;
            };
            if victim == k {
                break;
            }
            self.order.remove(&seq);
            if let Some(e) = self.map.remove(&victim) {
                self.used -= e.size;
            }
            evicted.push(victim);
        }
        evicted
    }
}

/// SSD disk tier: PNG files under `dir`, bounded by a byte budget with LRU
/// eviction driven by an in-memory index (no per-request directory scans).
struct DiskLru {
    dir: PathBuf,
    index: Mutex<ByteLru<()>>,
    tmp_seq: AtomicU64,
}

impl DiskLru {
    /// Open `dir` (creating it) and seed the LRU index from whatever is already
    /// on disk, ordered oldest-first by mtime so prior warmth survives restart.
    fn open(dir: PathBuf, budget: u64) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let mut found: Vec<(std::time::SystemTime, TileKey, u64)> = Vec::new();
        // Layout: {dir}/{z}/{x}/{y}_h{halo}.png
        for z_e in std::fs::read_dir(&dir)?.flatten() {
            if !z_e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            for x_e in std::fs::read_dir(z_e.path())?.flatten() {
                if !x_e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                for f in std::fs::read_dir(x_e.path())?.flatten() {
                    let p = f.path();
                    if let (Some(key), Ok(meta)) = (key_from_path(&dir, &p), f.metadata()) {
                        let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                        found.push((mtime, key, meta.len()));
                    }
                }
            }
        }
        found.sort_by_key(|(t, _, _)| *t);
        let mut index = ByteLru::new(budget);
        let mut evicted = Vec::new();
        for (_, key, size) in found {
            evicted.extend(index.put(key, size, ()));
        }
        let me = Self {
            dir,
            index: Mutex::new(index),
            tmp_seq: AtomicU64::new(0),
        };
        for k in evicted {
            me.remove_file(k);
        }
        Ok(me)
    }

    fn path(&self, k: TileKey) -> PathBuf {
        self.dir
            .join(format!("{}/{}/{}_h{}.png", k.z, k.x, k.y, k.halo))
    }

    fn remove_file(&self, k: TileKey) {
        let _ = std::fs::remove_file(self.path(k));
    }

    /// Read a cached tile (bumps its LRU recency). `None` = not cached. A stale
    /// index entry whose file vanished is dropped and treated as a miss.
    fn get(&self, k: TileKey) -> Option<Vec<u8>> {
        {
            let mut idx = self.index.lock().unwrap();
            idx.get(&k)?; // miss → don't touch disk
        }
        match std::fs::read(self.path(k)) {
            Ok(bytes) => Some(bytes),
            Err(_) => {
                // File gone under us — forget it so it gets re-rendered.
                let mut idx = self.index.lock().unwrap();
                idx.put(k, 0, ());
                idx.map.remove(&k);
                None
            }
        }
    }

    /// Write a tile through to disk (atomic tmp+rename) and record it, deleting
    /// any LRU victims that pushed us over budget.
    fn put(&self, k: TileKey, bytes: &[u8]) {
        let path = self.path(k);
        if let Some(parent) = path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return;
            }
        }
        let tmp = path.with_extension(format!(
            "tmp{}",
            self.tmp_seq.fetch_add(1, Ordering::Relaxed)
        ));
        if std::fs::write(&tmp, bytes).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        if std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        let evicted = {
            let mut idx = self.index.lock().unwrap();
            idx.put(k, bytes.len() as u64, ())
        };
        for v in evicted {
            self.remove_file(v);
        }
    }
}

/// Parse `{dir}/{z}/{x}/{y}_h{halo}.png` back into a [`TileKey`].
fn key_from_path(dir: &std::path::Path, p: &std::path::Path) -> Option<TileKey> {
    let rel = p.strip_prefix(dir).ok()?;
    let comps: Vec<_> = rel.components().collect();
    if comps.len() != 3 {
        return None;
    }
    let z: u8 = comps[0].as_os_str().to_str()?.parse().ok()?;
    let x: u32 = comps[1].as_os_str().to_str()?.parse().ok()?;
    let file = comps[2].as_os_str().to_str()?;
    let stem = file.strip_suffix(".png")?;
    let (y_s, halo_s) = stem.split_once("_h")?;
    Some(TileKey {
        z,
        x,
        y: y_s.parse().ok()?,
        halo: halo_s.parse().ok()?,
    })
}

/// Two-tier rendered-tile cache + render throttle. Cheap to `clone` (all `Arc`).
#[derive(Clone)]
pub struct DemTileCache {
    mem: Arc<Mutex<ByteLru<Arc<[u8]>>>>,
    disk: Option<Arc<DiskLru>>,
    render: Arc<Semaphore>,
    pub render_permits: usize,
}

impl DemTileCache {
    /// Build from env:
    ///   TILESERVER_DEM_TILE_CACHE_DIR        disk dir (unset → memory-only)
    ///   TILESERVER_DEM_TILE_CACHE_MEM_BYTES  RAM budget   (default 64 MiB)
    ///   TILESERVER_DEM_TILE_CACHE_DISK_BYTES disk budget  (default 3 GiB)
    ///   TILESERVER_DEM_RENDER_CONCURRENCY    max concurrent renders (default
    ///                                        = CPU cores); a miss past this 429s
    pub fn from_env() -> Self {
        fn bytes(var: &str, default: u64) -> u64 {
            std::env::var(var)
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&n| n > 0)
                .unwrap_or(default)
        }
        let mem_budget = bytes("TILESERVER_DEM_TILE_CACHE_MEM_BYTES", 64 * 1024 * 1024);
        let disk_budget = bytes(
            "TILESERVER_DEM_TILE_CACHE_DISK_BYTES",
            3 * 1024 * 1024 * 1024,
        );
        let permits = std::env::var("TILESERVER_DEM_RENDER_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
            });

        let disk = std::env::var("TILESERVER_DEM_TILE_CACHE_DIR").ok().filter(|s| !s.is_empty()).and_then(
            |d| match DiskLru::open(PathBuf::from(&d), disk_budget) {
                Ok(lru) => {
                    tracing::info!(dir = %d, budget_bytes = disk_budget, "dem tile disk cache ready");
                    Some(Arc::new(lru))
                }
                Err(e) => {
                    tracing::warn!(dir = %d, error = %e, "dem tile disk cache disabled (open failed)");
                    None
                }
            },
        );

        Self {
            mem: Arc::new(Mutex::new(ByteLru::new(mem_budget))),
            disk,
            render: Arc::new(Semaphore::new(permits)),
            render_permits: permits,
        }
    }

    /// Memory-tier lookup (fast, brief lock). Hits avoid the disk + the renderer.
    pub fn get_mem(&self, k: TileKey) -> Option<Arc<[u8]>> {
        self.mem.lock().unwrap().get(&k)
    }

    /// Disk-tier lookup (blocking file read — call from `spawn_blocking`).
    /// Promotes a hit into the memory tier.
    pub fn get_disk(&self, k: TileKey) -> Option<Arc<[u8]>> {
        let bytes = self.disk.as_ref()?.get(k)?;
        let arc: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
        self.mem
            .lock()
            .unwrap()
            .put(k, arc.len() as u64, arc.clone());
        Some(arc)
    }

    /// Write a freshly-rendered tile through both tiers (blocking — runs inside
    /// the render `spawn_blocking`).
    pub fn put(&self, k: TileKey, bytes: &[u8]) {
        let arc: Arc<[u8]> = Arc::from(bytes.to_vec().into_boxed_slice());
        self.mem.lock().unwrap().put(k, arc.len() as u64, arc);
        if let Some(disk) = &self.disk {
            disk.put(k, bytes);
        }
    }

    /// Try to reserve a render slot. `None` → the renderer is saturated; the
    /// caller should 429 rather than queue (this is the throttle).
    pub fn try_render(&self) -> Option<OwnedSemaphorePermit> {
        self.render.clone().try_acquire_owned().ok()
    }

    #[cfg(test)]
    fn for_test(permits: usize) -> Self {
        Self {
            mem: Arc::new(Mutex::new(ByteLru::new(1 << 20))),
            disk: None,
            render: Arc::new(Semaphore::new(permits)),
            render_permits: permits,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(y: u32) -> TileKey {
        TileKey {
            z: 13,
            x: 100,
            y,
            halo: 1,
        }
    }

    #[test]
    fn byte_lru_evicts_least_recently_used_over_budget() {
        let mut lru: ByteLru<u32> = ByteLru::new(100);
        assert!(lru.put(k(1), 40, 1).is_empty());
        assert!(lru.put(k(2), 40, 2).is_empty());
        // Touch #1 so #2 becomes LRU.
        assert_eq!(lru.get(&k(1)), Some(1));
        // #3 pushes us to 120 > 100 → evict the LRU (#2).
        let evicted = lru.put(k(3), 40, 3);
        assert_eq!(evicted, vec![k(2)]);
        assert_eq!(lru.get(&k(2)), None);
        assert_eq!(lru.get(&k(1)), Some(1));
        assert_eq!(lru.get(&k(3)), Some(3));
    }

    #[test]
    fn byte_lru_keeps_a_single_oversized_entry() {
        let mut lru: ByteLru<u32> = ByteLru::new(10);
        let evicted = lru.put(k(1), 999, 1);
        assert!(
            evicted.is_empty(),
            "must not evict the entry it just inserted"
        );
        assert_eq!(lru.get(&k(1)), Some(1));
    }

    #[test]
    fn disk_cache_round_trips_and_evicts_files() {
        let dir = std::env::temp_dir().join(format!("turbo-demcache-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let disk = DiskLru::open(dir.clone(), 100).unwrap();
        disk.put(k(1), &[0u8; 40]);
        disk.put(k(2), &[0u8; 40]);
        assert_eq!(disk.get(k(1)).map(|b| b.len()), Some(40));
        disk.get(k(1)); // bump #1
        disk.put(k(3), &[0u8; 40]); // over budget → evict LRU (#2)
        assert!(
            disk.get(k(2)).is_none(),
            "evicted tile must be gone from disk"
        );
        assert!(!disk.path(k(2)).exists(), "evicted file must be deleted");
        assert_eq!(disk.get(k(1)).map(|b| b.len()), Some(40));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_throttle_sheds_load_past_the_permit_cap() {
        // The guarantee: only `permits` concurrent renders; the next caller
        // gets None (→ the handler 429s) instead of queueing into collapse.
        let cache = DemTileCache::for_test(2);
        let p1 = cache.try_render();
        let p2 = cache.try_render();
        assert!(p1.is_some() && p2.is_some(), "first two renders admitted");
        assert!(cache.try_render().is_none(), "third is throttled (429)");
        drop(p1); // a render finished → a slot frees
        assert!(
            cache.try_render().is_some(),
            "freed slot admits the next render"
        );
    }

    #[test]
    fn disk_index_reseeds_from_existing_files_on_open() {
        let dir =
            std::env::temp_dir().join(format!("turbo-demcache-reseed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        {
            let disk = DiskLru::open(dir.clone(), 10_000).unwrap();
            disk.put(k(7), &[1u8; 50]);
        }
        // New instance: must rediscover the file from disk.
        let disk2 = DiskLru::open(dir.clone(), 10_000).unwrap();
        assert_eq!(disk2.get(k(7)).map(|b| b.len()), Some(50));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
