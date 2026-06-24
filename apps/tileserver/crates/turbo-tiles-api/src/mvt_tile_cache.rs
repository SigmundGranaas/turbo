//! Two-tier cache of rendered MVT vector tiles: a **small** in-RAM byte-LRU in
//! front of a **big** SSD byte-LRU, plus a render-concurrency limiter and a
//! bumpable data version for provision-time invalidation.
//!
//! The N50 basemap MVT is expensive to build — per request it runs
//! `ST_SimplifyPreserveTopology` + `ST_Transform` + `ST_AsMVT` across every
//! layer, over big Norwegian coastline/water polygons. The tiles are immutable
//! between provisions and the same tiles are requested repeatedly (many clients,
//! pans, client retries), so caching the rendered bytes turns a multi-second
//! query into a hit.
//!
//! The cache is two-tier, mirroring the DEM tile cache: a tiny RAM LRU (hot
//! tiles, microsecond hits) backed by a large SSD LRU. The disk tier is seeded
//! from whatever is already on disk at boot, so a restart (incl. an OOM-kill)
//! keeps prior warmth instead of re-rendering every coastal tile cold. Entries
//! are keyed by `version/key`; bumping the version on (re)provision makes the
//! previous generation unreachable (old files age out of the disk LRU).
//!
//! A render-concurrency semaphore bounds simultaneous cold renders so a burst of
//! distinct tiles can't congestion-collapse the DB or OOM the pod.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

struct Entry<V> {
    val: V,
    size: u64,
    seq: u64,
}

/// Byte-weighted LRU keyed by the full `version/resource/z/x/y` string. `order`
/// keeps keys by last-access so eviction pops the least-recently-used. `put`
/// returns the keys it evicted so the disk tier can delete their files.
struct ByteLru<V> {
    budget: u64,
    used: u64,
    clock: u64,
    map: HashMap<String, Entry<V>>,
    order: BTreeMap<u64, String>,
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

    fn get(&mut self, k: &str) -> Option<V> {
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

    fn put(&mut self, k: String, size: u64, val: V) -> Vec<String> {
        if let Some(old) = self.map.remove(&k) {
            self.used = self.used.saturating_sub(old.size);
            self.order.remove(&old.seq);
        }
        self.clock += 1;
        let seq = self.clock;
        self.order.insert(seq, k.clone());
        self.map.insert(k, Entry { val, size, seq });
        self.used += size;
        let mut evicted = Vec::new();
        while self.used > self.budget {
            let Some((&victim_seq, _)) = self.order.iter().next() else {
                break;
            };
            let victim = self.order.remove(&victim_seq).unwrap();
            if let Some(e) = self.map.remove(&victim) {
                self.used = self.used.saturating_sub(e.size);
            }
            evicted.push(victim);
        }
        evicted
    }

    fn forget(&mut self, k: &str) {
        if let Some(e) = self.map.remove(k) {
            self.used = self.used.saturating_sub(e.size);
            self.order.remove(&e.seq);
        }
    }
}

/// SSD disk tier: MVT files under `dir`, laid out at `{dir}/{version}/{key}.mvt`
/// (the key already contains `resource/z/x/y`), bounded by a byte budget with
/// LRU eviction driven by an in-memory index (no per-request directory scans).
struct DiskLru {
    dir: PathBuf,
    /// File extension for the cached payload (`mvt` for vector tiles, `png` for
    /// rendered raster tiles). Parameterised so the same machinery backs both.
    ext: &'static str,
    index: Mutex<ByteLru<()>>,
    tmp_seq: AtomicU64,
}

impl DiskLru {
    /// Open `dir` (creating it) and seed the LRU index from whatever is already
    /// on disk, ordered oldest-first by mtime so prior warmth survives restart.
    fn open(dir: PathBuf, budget: u64, ext: &'static str) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let mut found: Vec<(std::time::SystemTime, String, u64)> = Vec::new();
        collect_tiles(&dir, &dir, ext, &mut found)?;
        found.sort_by_key(|(t, _, _)| *t);
        let mut index = ByteLru::new(budget);
        let mut evicted = Vec::new();
        for (_, key, size) in found {
            evicted.extend(index.put(key, size, ()));
        }
        let me = Self {
            dir,
            ext,
            index: Mutex::new(index),
            tmp_seq: AtomicU64::new(0),
        };
        for k in evicted {
            me.remove_file(&k);
        }
        Ok(me)
    }

    fn path(&self, full_key: &str) -> PathBuf {
        self.dir.join(format!("{full_key}.{}", self.ext))
    }

    fn remove_file(&self, full_key: &str) {
        let _ = std::fs::remove_file(self.path(full_key));
    }

    /// Read a cached tile (bumps its LRU recency). `None` = not cached. A stale
    /// index entry whose file vanished is dropped and treated as a miss.
    fn get(&self, full_key: &str) -> Option<Vec<u8>> {
        {
            let mut idx = self.index.lock().unwrap();
            idx.get(full_key)?; // miss → don't touch disk
        }
        match std::fs::read(self.path(full_key)) {
            Ok(bytes) => Some(bytes),
            Err(_) => {
                self.index.lock().unwrap().forget(full_key);
                None
            }
        }
    }

    /// Write a tile through to disk (atomic tmp+rename) and record it, deleting
    /// any LRU victims that pushed us over budget.
    fn put(&self, full_key: &str, bytes: &[u8]) {
        let path = self.path(full_key);
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
            idx.put(full_key.to_string(), bytes.len() as u64, ())
        };
        for v in evicted {
            self.remove_file(&v);
        }
    }
}

/// Recursively collect `*.{ext}` files under `root`, recording (mtime, key, size)
/// where `key` is the path relative to `root` minus the `.{ext}` suffix.
fn collect_tiles(
    root: &Path,
    dir: &Path,
    ext: &str,
    out: &mut Vec<(std::time::SystemTime, String, u64)>,
) -> std::io::Result<()> {
    let suffix = format!(".{ext}");
    for e in std::fs::read_dir(dir)?.flatten() {
        let p = e.path();
        let ft = match e.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if ft.is_dir() {
            collect_tiles(root, &p, ext, out)?;
        } else if p.extension().and_then(|s| s.to_str()) == Some(ext) {
            if let (Ok(rel), Ok(meta)) = (p.strip_prefix(root), e.metadata()) {
                if let Some(key) = rel.to_str().and_then(|s| s.strip_suffix(&suffix)) {
                    let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                    out.push((mtime, key.to_string(), meta.len()));
                }
            }
        }
    }
    Ok(())
}

/// Rendered-MVT cache: small RAM LRU + big SSD LRU + render-concurrency limiter,
/// with a bumpable data version for provision-time invalidation. Cheaply
/// `Clone` (shared `Arc` internals) so it lives by value in the cloned
/// `ApiState`, exactly like the DEM tile cache.
#[derive(Clone)]
pub struct MvtTileCache {
    mem: Arc<Mutex<ByteLru<Arc<[u8]>>>>,
    disk: Option<Arc<DiskLru>>,
    render: Arc<Semaphore>,
    version: Arc<AtomicU64>,
    pub render_permits: usize,
}

impl MvtTileCache {
    /// Build the **MVT** vector-tile cache from env:
    ///   TILESERVER_MVT_TILE_CACHE_DIR        disk dir (unset → memory-only)
    ///   TILESERVER_MVT_TILE_CACHE_MEM_BYTES  RAM budget  (default 32 MiB — small,
    ///                                        just the hot set in front of disk)
    ///   TILESERVER_MVT_TILE_CACHE_DISK_BYTES disk budget (default 2 GiB)
    ///   TILESERVER_MVT_RENDER_CONCURRENCY    max concurrent cold renders
    ///                                        (default = CPU cores, min 2)
    pub fn from_env() -> Self {
        Self::from_env_with("MVT", "mvt")
    }

    /// Build the **raster** PNG-tile cache from the `TILESERVER_RASTER_*`
    /// counterparts of the MVT vars. Same two-tier LRU + render-concurrency
    /// limiter; only the disk extension (`png`) and env prefix differ. Server-
    /// side raster rendering is even more expensive than MVT (it rasterises the
    /// same layers, plus hillshade), and low-zoom tiles can exceed the default
    /// statement timeout cold — caching the bytes turns that into a one-time cost.
    pub fn png_from_env() -> Self {
        Self::from_env_with("RASTER", "png")
    }

    /// Shared constructor. `prefix` selects the `TILESERVER_{prefix}_*` env vars;
    /// `ext` is the on-disk file extension and cache label.
    fn from_env_with(prefix: &str, ext: &'static str) -> Self {
        fn bytes(var: &str, default: u64) -> u64 {
            std::env::var(var)
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&n| n > 0)
                .unwrap_or(default)
        }
        let mem_budget = bytes(
            &format!("TILESERVER_{prefix}_TILE_CACHE_MEM_BYTES"),
            32 * 1024 * 1024,
        );
        let disk_budget = bytes(
            &format!("TILESERVER_{prefix}_TILE_CACHE_DISK_BYTES"),
            2 * 1024 * 1024 * 1024,
        );
        let permits = std::env::var(format!("TILESERVER_{prefix}_RENDER_CONCURRENCY"))
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
            })
            .max(2);

        let disk = std::env::var(format!("TILESERVER_{prefix}_TILE_CACHE_DIR"))
            .ok()
            .filter(|s| !s.is_empty())
            .and_then(|d| match DiskLru::open(PathBuf::from(&d), disk_budget, ext) {
                Ok(lru) => {
                    tracing::info!(dir = %d, budget_bytes = disk_budget, label = ext, "tile disk cache ready");
                    Some(Arc::new(lru))
                }
                Err(e) => {
                    tracing::warn!(dir = %d, error = %e, label = ext, "tile disk cache disabled (open failed)");
                    None
                }
            });

        tracing::info!(
            mem_budget_bytes = mem_budget,
            disk_budget_bytes = disk_budget,
            render_permits = permits,
            disk = disk.is_some(),
            label = ext,
            "tile cache ready"
        );
        Self {
            mem: Arc::new(Mutex::new(ByteLru::new(mem_budget))),
            disk,
            render: Arc::new(Semaphore::new(permits)),
            version: Arc::new(AtomicU64::new(0)),
            render_permits: permits,
        }
    }

    fn full_key(&self, key: &str) -> String {
        format!("{}/{key}", self.version.load(Ordering::Relaxed))
    }

    /// Memory-tier lookup (fast, brief lock). A hit skips the disk + the renderer.
    pub fn get_mem(&self, key: &str) -> Option<Arc<[u8]>> {
        let fk = self.full_key(key);
        self.mem.lock().unwrap().get(&fk)
    }

    /// Disk-tier lookup (blocking file read — call from `spawn_blocking`).
    /// Promotes a hit into the memory tier.
    pub fn get_disk(&self, key: &str) -> Option<Arc<[u8]>> {
        let fk = self.full_key(key);
        let bytes = self.disk.as_ref()?.get(&fk)?;
        let arc: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
        self.mem
            .lock()
            .unwrap()
            .put(fk, arc.len() as u64, arc.clone());
        Some(arc)
    }

    /// Store a freshly-rendered tile through both tiers. The disk write is
    /// blocking, so call from `spawn_blocking` (the byte payload is small, but
    /// the rename still shouldn't run on the async executor).
    pub fn put(&self, key: &str, bytes: Arc<[u8]>) {
        let fk = self.full_key(key);
        self.mem
            .lock()
            .unwrap()
            .put(fk.clone(), bytes.len() as u64, bytes.clone());
        if let Some(disk) = &self.disk {
            disk.put(&fk, &bytes);
        }
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

    /// Serve `key` from the mem→disk tiers, or render it once and cache it
    /// through both. `render` is awaited only on a full miss, under a render
    /// permit, after a re-check (so a thundering herd for one cold tile collapses
    /// to a single DB render). Disk read + write run on `spawn_blocking` so the
    /// file I/O never stalls the async executor.
    pub async fn get_or_render<F, Fut, E>(&self, key: String, render: F) -> Result<Arc<[u8]>, E>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<u8>, E>>,
    {
        if let Some(b) = self.get_mem(&key) {
            return Ok(b);
        }
        {
            let this = self.clone();
            let dk = key.clone();
            if let Ok(Some(b)) = tokio::task::spawn_blocking(move || this.get_disk(&dk)).await {
                return Ok(b);
            }
        }
        let _permit = self.acquire_render().await;
        // Another request may have rendered + cached it while we queued.
        if let Some(b) = self.get_mem(&key) {
            return Ok(b);
        }
        let rendered = render().await?;
        let arc: Arc<[u8]> = Arc::from(rendered.into_boxed_slice());
        {
            let this = self.clone();
            let pk = key.clone();
            let pa = arc.clone();
            let _ = tokio::task::spawn_blocking(move || this.put(&pk, pa)).await;
        }
        Ok(arc)
    }

    /// Invalidate every cached tile (call after a (re)provision changes the
    /// underlying data) by advancing the version the keys are namespaced under.
    /// Old-generation disk files become unreachable and age out of the disk LRU.
    pub fn bump_version(&self) {
        let v = self.version.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::info!(version = v, "mvt tile cache invalidated (data changed)");
    }
}
