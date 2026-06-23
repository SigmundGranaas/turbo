//! In-memory cache of rendered MVT vector tiles.
//!
//! The N50 basemap MVT is expensive to build — per request it runs
//! `ST_SimplifyPreserveTopology` + `ST_Transform` + `ST_AsMVT` across every
//! layer, over big Norwegian coastline/water polygons (~0.2–1.5 s per tile warm,
//! worse under concurrency on a 2-core pod). The tiles are immutable between
//! provisions and the same tiles are requested repeatedly (many clients, pans,
//! client retries), so caching the rendered bytes turns a multi-second query
//! into a ~microsecond memory hit — the same idea as the DEM tile cache.
//!
//! A small render-concurrency semaphore bounds simultaneous cold renders so a
//! burst of distinct tiles can't congestion-collapse the DB (each miss holds a
//! permit for its query). Cache entries are keyed by `version/key`; bumping the
//! version on (re)provision makes the previous generation unreachable, so stale
//! tiles are never served after the data changes.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

struct Entry {
    val: Arc<[u8]>,
    size: u64,
    seq: u64,
}

/// Byte-weighted LRU keyed by the full `version/resource/z/x/y` string. `order`
/// keeps keys by last-access so eviction pops the least-recently-used.
struct ByteLru {
    budget: u64,
    used: u64,
    clock: u64,
    map: HashMap<String, Entry>,
    order: BTreeMap<u64, String>,
}

impl ByteLru {
    fn new(budget: u64) -> Self {
        Self {
            budget,
            used: 0,
            clock: 0,
            map: HashMap::new(),
            order: BTreeMap::new(),
        }
    }

    fn get(&mut self, k: &str) -> Option<Arc<[u8]>> {
        let (old_seq, val) = {
            let e = self.map.get(k)?;
            (e.seq, e.val.clone())
        };
        self.order.remove(&old_seq);
        self.clock += 1;
        let seq = self.clock;
        self.order.insert(seq, k.to_string());
        self.map.get_mut(k).unwrap().seq = seq;
        Some(val)
    }

    fn put(&mut self, k: String, val: Arc<[u8]>) {
        let size = val.len() as u64;
        if let Some(old) = self.map.remove(&k) {
            self.used = self.used.saturating_sub(old.size);
            self.order.remove(&old.seq);
        }
        self.clock += 1;
        let seq = self.clock;
        self.order.insert(seq, k.clone());
        self.map.insert(k, Entry { val, size, seq });
        self.used += size;
        while self.used > self.budget {
            let Some((&victim_seq, _)) = self.order.iter().next() else {
                break;
            };
            let victim = self.order.remove(&victim_seq).unwrap();
            if let Some(e) = self.map.remove(&victim) {
                self.used = self.used.saturating_sub(e.size);
            }
        }
    }
}

/// Rendered-MVT cache: a byte-budgeted memory LRU + a render-concurrency
/// limiter, with a bumpable data version for provision-time invalidation.
/// Cheaply `Clone` (shared `Arc` internals) so it lives by value in the cloned
/// `ApiState`, exactly like the DEM tile cache.
#[derive(Clone)]
pub struct MvtTileCache {
    mem: Arc<Mutex<ByteLru>>,
    render: Arc<Semaphore>,
    version: Arc<AtomicU64>,
    pub render_permits: usize,
}

impl MvtTileCache {
    /// Build from env:
    ///   TILESERVER_MVT_TILE_CACHE_MEM_BYTES  RAM budget (default 128 MiB)
    ///   TILESERVER_MVT_RENDER_CONCURRENCY    max concurrent cold renders
    ///                                        (default = CPU cores, min 2)
    pub fn from_env() -> Self {
        let mem_budget = std::env::var("TILESERVER_MVT_TILE_CACHE_MEM_BYTES")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(128 * 1024 * 1024);
        let permits = std::env::var("TILESERVER_MVT_RENDER_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
            })
            .max(2);
        tracing::info!(
            mem_budget_bytes = mem_budget,
            render_permits = permits,
            "mvt tile cache ready"
        );
        Self {
            mem: Arc::new(Mutex::new(ByteLru::new(mem_budget))),
            render: Arc::new(Semaphore::new(permits)),
            version: Arc::new(AtomicU64::new(0)),
            render_permits: permits,
        }
    }

    fn full_key(&self, key: &str) -> String {
        format!("{}/{key}", self.version.load(Ordering::Relaxed))
    }

    /// Memory lookup (brief lock). A hit skips the renderer entirely.
    pub fn get(&self, key: &str) -> Option<Arc<[u8]>> {
        let fk = self.full_key(key);
        self.mem.lock().unwrap().get(&fk)
    }

    /// Store a freshly-rendered tile.
    pub fn put(&self, key: &str, bytes: Arc<[u8]>) {
        let fk = self.full_key(key);
        self.mem.lock().unwrap().put(fk, bytes);
    }

    /// Reserve a render slot (awaits if all permits are busy), bounding the
    /// number of simultaneous cold DB renders. Held for the duration of the
    /// render so a burst of misses queues instead of stampeding the DB.
    pub async fn acquire_render(&self) -> OwnedSemaphorePermit {
        self.render
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore open")
    }

    /// Invalidate every cached tile (call after a (re)provision changes the
    /// underlying data) by advancing the version the keys are namespaced under.
    pub fn bump_version(&self) {
        let v = self.version.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::info!(version = v, "mvt tile cache invalidated (data changed)");
    }
}
