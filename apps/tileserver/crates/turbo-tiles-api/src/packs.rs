//! Building and caching region packs, so `/v1/packs/...` can serve them.
//!
//! A pack is derived, deterministic and expensive: the same key always
//! yields the same bytes, and producing them costs a slice of the
//! national artifacts. That combination makes it a cache problem, not a
//! request problem — build once, serve forever, and make sure two
//! simultaneous requests for the same key produce one build rather than
//! two.
//!
//! # Why the origin builds at all
//!
//! The alternative is pre-building the country. Norway at the pack grid
//! is tens of thousands of cells and, as *regions* rather than cells,
//! not enumerable at all — a region is any rectangle of cells, so the
//! set is quadratic in a space that is already large. On-demand with a
//! durable cache is the only shape that fits a key space defined by what
//! users actually ask for.
//!
//! # What protects it
//!
//! Three things, because this endpoint turns a GET into arbitrary CPU:
//!
//! - a **cell cap**, so no single request can ask for the country;
//! - a **global permit**, so N requests cannot each take a core; and
//! - a **per-key lock**, so the same region is never built twice at once.
//!
//! The first is the one that matters most: without it the endpoint is a
//! denial-of-service primitive with a friendly URL.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use turbo_geodata_pack::{PackKey, Region};

/// Terrain kept beyond the requested region. Matches the CLI default;
/// E10 measured 500 m as sufficient and this leaves margin.
const HALO_M: f64 = 1000.0;

/// Largest region a single request may ask for, in grid cells.
///
/// 400 z12 cells is roughly 76 x 76 km at 67°N — comfortably more than
/// the app's own download dialog allows, and far less than a request
/// that would wedge the server for minutes. A cap the client never hits
/// and an attacker always does.
const MAX_CELLS: u64 = 400;

/// How long a request waits for a build before being told to come back.
///
/// Past this the client gets `202` + `Retry-After` and the build keeps
/// running. Long enough that an ordinary region completes inline; short
/// enough that nothing sits on a socket for minutes.
const BUILD_WAIT: std::time::Duration = std::time::Duration::from_secs(20);

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("{0}")]
    BadKey(String),
    #[error("region too large: {cells} cells, limit {MAX_CELLS}")]
    TooLarge { cells: u64 },
    #[error("pack build failed: {0}")]
    Build(String),
    /// Not an error — the build is running, come back.
    #[error("building")]
    Building,
}

/// Builds packs on demand and caches them on disk.
pub struct PackService {
    /// The national artifacts to slice from.
    src: PathBuf,
    /// Where built packs live. Wants its own volume: a full pack cache
    /// must never be able to wedge the artifacts the router mmaps.
    cache: PathBuf,
    /// Caps concurrent builds. Slicing is CPU- and IO-heavy and this
    /// process is also serving routes.
    permits: Arc<tokio::sync::Semaphore>,
    /// In-flight builds, keyed by pack key. The receiver resolves when
    /// the build finishes; late arrivals wait on it instead of starting
    /// a second one. `Arc` because the build task outlives the request
    /// that started it and has to remove its own entry.
    inflight: Arc<Mutex<HashMap<String, BuildWatch>>>,
}

type BuildWatch = tokio::sync::watch::Receiver<Option<Result<(), String>>>;

impl PackService {
    pub fn new(src: PathBuf, cache: PathBuf, max_concurrent_builds: usize) -> Self {
        Self {
            src,
            cache,
            permits: Arc::new(tokio::sync::Semaphore::new(max_concurrent_builds.max(1))),
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Directory a built pack lives in.
    fn dir(&self, key: &PackKey) -> PathBuf {
        self.cache.join(key.to_string())
    }

    /// Is this pack already built?
    ///
    /// Judged by the manifest, which `build` writes **last** — so a
    /// directory with one is a directory whose contents were sliced and
    /// verified. An interrupted build leaves artifacts without a
    /// manifest and is correctly treated as absent.
    fn is_built(&self, key: &PackKey) -> bool {
        self.dir(key)
            .join(turbo_tiles_artifacts::PackManifest::FILENAME)
            .is_file()
    }

    /// Resolve a key string, enforcing the cell cap.
    pub fn parse_key(&self, s: &str) -> Result<PackKey, PackError> {
        let key = PackKey::parse(s)
            .ok_or_else(|| PackError::BadKey(format!("malformed pack key '{s}'")))?;
        let cells = key.cells();
        if cells > MAX_CELLS {
            return Err(PackError::TooLarge { cells });
        }
        Ok(key)
    }

    /// Path to `file` within this pack, building the pack if needed.
    ///
    /// Returns [`PackError::Building`] when the build outlives
    /// [`BUILD_WAIT`]; the caller turns that into `202` + `Retry-After`.
    pub async fn file(&self, key: &PackKey, file: &str) -> Result<PathBuf, PackError> {
        // Path traversal: a filename, never a path. `norway.dem` yes,
        // `../../etc/passwd` no. Rejecting rather than sanitising —
        // there is no legitimate request this refuses.
        if file.is_empty() || file.contains('/') || file.contains('\\') || file.contains("..") {
            return Err(PackError::BadKey(format!("bad file name '{file}'")));
        }

        if !self.is_built(key) {
            self.ensure_built(key).await?;
        }

        let path = self.dir(key).join(file);
        if !path.is_file() {
            return Err(PackError::BadKey(format!("pack has no file '{file}'")));
        }
        Ok(path)
    }

    /// Build the pack, or join the build already running for this key.
    async fn ensure_built(&self, key: &PackKey) -> Result<(), PackError> {
        let name = key.to_string();

        // Join an existing build, or register this one. Done under one
        // lock so two requests arriving together cannot both decide they
        // are the builder.
        let mut rx = {
            let mut map = self.inflight.lock().unwrap();
            if let Some(rx) = map.get(&name) {
                rx.clone()
            } else {
                let (tx, rx) = tokio::sync::watch::channel(None);
                map.insert(name.clone(), rx.clone());
                self.spawn_build(*key, name.clone(), tx);
                rx
            }
        };

        // Wait, but not forever.
        let waited = tokio::time::timeout(BUILD_WAIT, async {
            // `changed()` resolves on the first send after this point;
            // the initial `None` is never sent, so there is no race with
            // a build that finished between registration and here — the
            // borrow below re-checks the current value either way.
            if rx.borrow().is_none() {
                let _ = rx.changed().await;
            }
            rx.borrow().clone()
        })
        .await;

        match waited {
            Err(_) => Err(PackError::Building),
            Ok(None) => Err(PackError::Building),
            Ok(Some(Ok(()))) => Ok(()),
            Ok(Some(Err(e))) => Err(PackError::Build(e)),
        }
    }

    fn spawn_build(
        &self,
        key: PackKey,
        name: String,
        tx: tokio::sync::watch::Sender<Option<Result<(), String>>>,
    ) {
        let src = self.src.clone();
        let dst = self.dir(&key);
        let permits = self.permits.clone();
        let inflight = self.inflight.clone();

        // Detached on purpose. A client that gives up (or times out into
        // a 202) must not cancel the work — the next request would start
        // it again from nothing, and for a slow region that is a loop
        // that never converges.
        tokio::spawn(async move {
            let _permit = permits.acquire().await;
            let result = tokio::task::spawn_blocking(move || build_one(&src, &dst, &key))
                .await
                .unwrap_or_else(|e| Err(format!("build task panicked: {e}")));
            if let Err(e) = &result {
                tracing::error!(pack = %name, error = %e, "pack build failed");
            }
            let _ = tx.send(Some(result));
            // Forget the build. A success is now discoverable on disk,
            // and a FAILURE must be retried rather than remembered —
            // leaving the entry would make one transient error (a full
            // disk, a torn artifact read) permanent for that region
            // until the process restarts.
            inflight.lock().unwrap().remove(&name);
        });
    }
}

/// Slice one pack into a temp directory and rename it into place.
///
/// The rename is what makes a half-built pack impossible to observe: a
/// reader either sees no directory or sees a complete one. Building
/// straight into the final path would let a crash leave artifacts
/// without a manifest — recoverable, since `is_built` checks the
/// manifest, but it would also let a *reader* see a partial DEM through
/// a directory listing.
fn build_one(src: &Path, dst: &Path, key: &PackKey) -> Result<(), String> {
    let [min_lon, min_lat, max_lon, max_lat] = key.extent();
    let tmp = dst.with_extension("partial");
    let _ = std::fs::remove_dir_all(&tmp);

    let started = std::time::Instant::now();
    turbo_geodata_pack::build(
        src,
        &tmp,
        &Region::Bbox([min_lon, min_lat, max_lon, max_lat]),
        HALO_M,
    )
    .map_err(|e| e.to_string())?;

    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_dir_all(dst);
    std::fs::rename(&tmp, dst).map_err(|e| e.to_string())?;

    tracing::info!(
        pack = %key,
        cells = key.cells(),
        ms = started.elapsed().as_millis() as u64,
        "built pack"
    );
    Ok(())
}
