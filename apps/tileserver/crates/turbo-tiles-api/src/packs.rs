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
//! - an **area cap**, so no single request can ask for the country;
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

/// Largest region a single request may ask for, in **ground area**.
///
/// # Why not cells
///
/// This cap was 400 z12 cells, described as "roughly 76 x 76 km at
/// 67°N". That description was accurate and the cap was still wrong,
/// because a cell is a Mercator cell: it is square on the ground at
/// every latitude, but its *size* falls as latitude rises. The same 400
/// cells are 5 842 km² at 67°N and **11 017 km²** at 58°N — and Norway
/// runs from 58 to 71, so the cap was loosest exactly where most of the
/// country is.
///
/// M2 measured what that costs. Cutting 84.9 x 99.0 km (8 405 km²) out
/// of the Sjunkhatten source took **12.0 s** and produced **122 MB**;
/// the phases that scale with the region were 10.9 s of it. Scaled to
/// 11 017 km² and with the graph phase against national rather than
/// regional artifacts, a southern request at the old cap was a ~161 MB
/// pack and ~18 s of build — inside [`BUILD_WAIT`] by 10%, and a
/// download no phone on a mountain connection should be offered as one
/// indivisible unit.
///
/// # Where the number comes from
///
/// 5 500 km² keeps the build near 11 s and the pack near 80 MB at every
/// Norwegian latitude. It is deliberately close to what 400 cells meant
/// at 67°N — the intent behind the old constant was right; only its
/// units were not.
const MAX_AREA_SQ_KM: f64 = 5_500.0;

/// A cheap pre-filter, applied before any trigonometry.
///
/// A key naming an absurd span is rejected on arithmetic alone. The
/// area cap is the real bound; this one exists so a malformed or
/// hostile key never reaches the projection maths.
const MAX_CELLS: u64 = 4_096;

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
    /// The region is bigger than one pack may cover.
    ///
    /// Carries km² rather than cells because that is the number the
    /// client can act on: it computes its own area with the same
    /// formula and never asks for what it would be refused.
    #[error("region too large: {area_sq_km:.0} km², limit {MAX_AREA_SQ_KM:.0} km²")]
    TooLarge { area_sq_km: f64 },
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

    /// Resolve a key string, enforcing the size caps.
    pub fn parse_key(&self, s: &str) -> Result<PackKey, PackError> {
        let key = PackKey::parse(s)
            .ok_or_else(|| PackError::BadKey(format!("malformed pack key '{s}'")))?;
        if key.cells() > MAX_CELLS {
            return Err(PackError::TooLarge {
                area_sq_km: f64::INFINITY,
            });
        }
        let area_sq_km = key.area_sq_km();
        if area_sq_km > MAX_AREA_SQ_KM {
            return Err(PackError::TooLarge { area_sq_km });
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Measured (M2): 122.4 MB over 8 405 km², cutting real terrain.
    const KB_PER_SQ_KM: f64 = 14.6;

    /// What `MAX_AREA_SQ_KM` costs, stated in the units it bounds.
    ///
    /// Nobody operating this server cares about km² either. They care
    /// about two numbers the cap implies and does not say: how long a
    /// build at the cap takes, and how big the pack a phone is then
    /// offered. Both were measured; both are easy to invalidate by
    /// editing one constant; so both are asserted.
    ///
    /// Cutting 84.9 x 99.0 km (8 405 km²) out of the Sjunkhatten source
    /// took 12.0 s, of which 10.9 s was in phases that scale with the
    /// region. The graph phase costs ~2.9 s more against national
    /// artifacts than regional ones (measured separately: 5.9 M edges,
    /// cold), and the rest is roughly fixed.
    #[test]
    fn the_cap_bounds_a_build_that_fits_the_wait_and_a_pack_that_fits_a_phone() {
        let build_s = 10.9 * (MAX_AREA_SQ_KM / 8_405.0) + 1.1 + 2.9;
        assert!(
            build_s < BUILD_WAIT.as_secs_f64() * 0.7,
            "the largest region this cap admits needs ~{build_s:.0} s of the {}s a request \
             waits inline. Not an outage — the 202 path handles it — but with this little \
             headroom the 202 becomes the common case, which is not what BUILD_WAIT is for.",
            BUILD_WAIT.as_secs()
        );

        let mb = MAX_AREA_SQ_KM * KB_PER_SQ_KM / 1000.0;
        assert!(
            mb < 100.0,
            "the cap admits a {mb:.0} MB pack — past this the phone is the problem, not the \
             server: it is minutes on a mountain connection, offered as one indivisible unit"
        );
    }

    /// The bug this cap's units were hiding.
    ///
    /// The old cap was 400 cells, reasoned about at 67°N where it means
    /// 5 842 km². A z12 cell is square on the ground — the cos(lat) that
    /// shrinks a degree of longitude is the same one that stretches the
    /// Mercator y scale — but it still *shrinks* with latitude, so the
    /// same 400 cells are 11 017 km² at 58°N. Norway runs 58 to 71, and
    /// the cap was loosest where most of the country is.
    ///
    /// This is the regression test for that: whatever the cap is
    /// expressed in, it must admit the same amount of WORK everywhere.
    #[test]
    fn the_cap_admits_the_same_work_at_every_norwegian_latitude() {
        let svc = PackService::new(PathBuf::from("/nonexistent"), PathBuf::from("/tmp"), 1);
        let mut areas = Vec::new();

        // Lindesnes to Nordkapp.
        for lat in [58.0, 62.0, 67.0, 71.0] {
            let one = PackKey::covering(15.0, lat, 15.0, lat, turbo_geodata_pack::PACK_GRID_Z);
            let block = |side: u32| PackKey {
                z: one.z,
                x0: one.x0,
                y0: one.y0,
                x1: one.x0 + side - 1,
                y1: one.y0 + side - 1,
            };
            // Grow until the SERVICE refuses. Asking `parse_key` rather
            // than recomputing the area here is the whole point: this
            // test must measure the cap that ships, not a second copy of
            // the formula that would agree with it by construction.
            let mut side = 1u32;
            while side < 200 && svc.parse_key(&block(side + 1).to_string()).is_ok() {
                side += 1;
            }
            assert!(side > 1, "nothing at all was admissible at {lat}degN");
            areas.push(block(side).area_sq_km());
        }

        let (lo, hi) = (
            areas.iter().cloned().fold(f64::INFINITY, f64::min),
            areas.iter().cloned().fold(0.0, f64::max),
        );
        // Cells are discrete, so the largest admissible block cannot sit
        // exactly on the cap at every latitude — but it must land in the
        // same neighbourhood, not vary by the 1.9x a cell cap gave.
        assert!(
            hi / lo < 1.35,
            "the cap admits {hi:.0} km² at one Norwegian latitude and {lo:.0} km² at another \
             ({:.1}x) — it is bounding cells again, not work. Areas: {areas:?}",
            hi / lo
        );
    }

    /// A region past the cap is refused, and the refusal says km².
    #[test]
    fn an_oversized_region_is_refused_in_the_clients_units() {
        let svc = PackService::new(PathBuf::from("/nonexistent"), PathBuf::from("/tmp"), 1);
        // 30 x 30 cells at 58degN: ~24 000 km², comfortably past the cap.
        let one = PackKey::covering(15.0, 58.0, 15.0, 58.0, turbo_geodata_pack::PACK_GRID_Z);
        let big = PackKey {
            z: one.z,
            x0: one.x0,
            y0: one.y0,
            x1: one.x0 + 29,
            y1: one.y0 + 29,
        };
        match svc.parse_key(&big.to_string()) {
            Err(PackError::TooLarge { area_sq_km }) => {
                assert!(area_sq_km > MAX_AREA_SQ_KM, "got {area_sq_km:.0} km²");
                assert!(
                    area_sq_km.is_finite(),
                    "the area cap, not the shape pre-filter, should have caught this"
                );
            }
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }
}
