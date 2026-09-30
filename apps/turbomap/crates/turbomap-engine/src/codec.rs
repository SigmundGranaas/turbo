//! The decode queue — image decode OFF the render thread (plan B4.1).
//!
//! Before this, `ingest_raster_encoded`/`ingest_terrain_encoded` ran
//! `image::load_from_memory` on the calling thread — the render thread on
//! Android (mitigated by time-slicing the ingest drain), the main thread on
//! web. Now `ingest_*` only *accepts bytes*: decode runs on the host's
//! [`turbomap_core::work::Executor`] (native) or inline under the apply
//! budget (wasm has no threads),
//! and the decoded RGBA is applied to the GPU caches at the top of
//! `render()`, bounded per frame by the tiered apply budget
//! ([`APPLY_BUDGET_MOVING`] / [`APPLY_BUDGET_SETTLED`]).
//!
//! Contract points hosts rely on:
//! - A tile stays in `pending_tiles` until its decode *applies* — so the
//!   queue dedups enqueued keys, or hosts would refetch every in-flight
//!   tile each reconcile pass (the 30k-backlog bug, engine edition).
//! - [`DecodeQueue::backlog`] must count as "animating": render-on-demand
//!   hosts keep pumping frames until the queue is empty, or the last tiles
//!   would only appear on the next unrelated invalidation.
//! - Decode failures clear the dedup entry and are dropped: the tile goes
//!   back to pending and the host's normal retry/backoff owns the policy.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use turbomap_core::{dem::DecodedDem, DemEncoding, TileId, VectorStyle};
use web_time::Instant;

/// Per-frame wall-time budgets for applying decoded tiles (GPU upload +
/// bookkeeping; on wasm also the decode itself). Two tiers, chosen by
/// whether the CAMERA is animating (visual motion — fades don't count,
/// they ARE applies arriving):
/// - moving: tight, so an ease/fling never hitches on tile uploads;
/// - settled: generous, so a cold load's ~hundreds-of-tiles working set
///   catches up within the settle instead of starving for whole seconds
///   behind a 6 ms trickle (the sim's shadow-stall gate caught exactly
///   that: `bl=true` on every frame, pans measured mid-cold-load).
pub(crate) const APPLY_BUDGET_MOVING: Duration = Duration::from_millis(6);
pub(crate) const APPLY_BUDGET_SETTLED: Duration = Duration::from_millis(32);

/// What a decode job is for — also the dedup key.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum QueueKey {
    Raster { layer_id: String, tile: TileId },
    Terrain { tile: TileId },
    Vector { layer_id: String, tile: TileId },
}

pub(crate) struct DecodeJob {
    pub key: QueueKey,
    pub bytes: Vec<u8>,
    /// Vector jobs only: the layer's style at enqueue time plus its epoch.
    /// Tessellation bakes the style into the mesh, so the worker needs the
    /// style value and the apply side must reject a result whose epoch no
    /// longer matches the layer (repaint/rebuild raced the decode).
    pub style: Option<(Arc<VectorStyle>, u64)>,
    /// Terrain jobs only: the source's declared RGB→metres encoding. This
    /// is where the DEM codec runs (plan D3) — the worker decodes to real
    /// heights and the render path never sees an encoding.
    pub dem_encoding: Option<DemEncoding>,
}

impl QueueKey {
    /// For a failure message: which layer and tile.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    fn describe(&self) -> String {
        match self {
            QueueKey::Raster { layer_id, tile } => {
                format!("raster layer {layer_id:?} tile {tile:?}")
            }
            QueueKey::Terrain { tile } => format!("terrain tile {tile:?}"),
            QueueKey::Vector { layer_id, tile } => {
                format!("vector layer {layer_id:?} tile {tile:?}")
            }
        }
    }
}

/// A decoded, ready-to-apply tile.
pub(crate) struct Decoded {
    pub key: QueueKey,
    pub kind: DecodedKind,
}

pub(crate) enum DecodedKind {
    /// Raster RGBA ready for GPU upload.
    Image { rgba: Vec<u8>, w: u32, h: u32 },
    /// A DEM tile decoded to real heights + coverage (the codec ran here,
    /// off the render thread — the caches only ever see metres).
    Dem { dem: DecodedDem },
    /// A tessellated vector tile + the style epoch it was built against.
    Vector {
        out: turbomap_core::tessellate::TessellationOutput,
        epoch: u64,
    },
}

fn decode(job: DecodeJob) -> (QueueKey, Option<Decoded>) {
    let DecodeJob {
        key,
        bytes,
        style,
        dem_encoding,
    } = job;
    match &key {
        QueueKey::Raster { .. } => match image::load_from_memory(&bytes) {
            Ok(img) => {
                let img = img.to_rgba8();
                let (w, h) = img.dimensions();
                let kind = DecodedKind::Image {
                    rgba: img.into_raw(),
                    w,
                    h,
                };
                (key.clone(), Some(Decoded { key, kind }))
            }
            Err(_) => (key, None),
        },
        QueueKey::Terrain { .. } => {
            let enc = dem_encoding.unwrap_or(DemEncoding::MapboxRgb);
            match image::load_from_memory(&bytes) {
                Ok(img) => {
                    let img = img.to_rgba8();
                    let (w, h) = img.dimensions();
                    match turbomap_core::decode_dem_rgba(img.as_raw(), w, h, enc) {
                        Some(dem) => {
                            let kind = DecodedKind::Dem { dem };
                            (key.clone(), Some(Decoded { key, kind }))
                        }
                        None => (key, None),
                    }
                }
                Err(_) => (key, None),
            }
        }
        QueueKey::Vector { tile, .. } => {
            let Some((style, epoch)) = style else {
                return (key, None);
            };
            match turbomap_core::vector::decode_mvt(&bytes) {
                Ok(vtile) => {
                    let out = turbomap_core::tessellate::tessellate(*tile, &vtile, &style);
                    let kind = DecodedKind::Vector { out, epoch };
                    (key.clone(), Some(Decoded { key, kind }))
                }
                Err(_) => (key, None),
            }
        }
    }
}

// ---- native: the host's executor ----------------------------------------

/// What comes back from a job: its result, or word that it never ran.
#[cfg(not(target_arch = "wasm32"))]
enum Outcome {
    Done(QueueKey, Option<Decoded>),
    Lost(QueueKey),
}

/// Rides inside a job and reports it [`Outcome::Lost`] if the job is
/// dropped without running, or unwinds out of its decode. The executor
/// contract ("every job runs") is checked here rather than trusted.
#[cfg(not(target_arch = "wasm32"))]
struct Receipt {
    key: Option<QueueKey>,
    results: crossbeam_channel::Sender<Outcome>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Receipt {
    fn drop(&mut self) {
        if let Some(key) = self.key.take() {
            // A closed channel means the engine is gone, and nobody is
            // waiting for this tile.
            let _ = self.results.send(Outcome::Lost(key));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct DecodeQueue {
    executor: Arc<dyn turbomap_core::work::Executor>,
    results_tx: crossbeam_channel::Sender<Outcome>,
    results: crossbeam_channel::Receiver<Outcome>,
    /// Keys enqueued and not yet applied/failed — the dedup set. Only the
    /// engine's thread touches it (`&mut self` API), so no lock.
    in_flight: HashSet<QueueKey>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DecodeQueue {
    /// Decodes run on `executor`, which the host owns (see
    /// [`turbomap_core::work`]). The engine spawns no thread of its own.
    pub fn new(executor: Arc<dyn turbomap_core::work::Executor>) -> Self {
        let (results_tx, results) = crossbeam_channel::unbounded();
        Self {
            executor,
            results_tx,
            results,
            in_flight: HashSet::new(),
        }
    }

    /// Accept bytes for decode. Returns `false` (and drops the bytes) if
    /// this key is already in flight — the dedup that keeps a host's
    /// reconcile loop from re-decoding every not-yet-applied tile.
    pub fn enqueue(
        &mut self,
        key: QueueKey,
        bytes: Vec<u8>,
        style: Option<(Arc<VectorStyle>, u64)>,
        dem_encoding: Option<DemEncoding>,
    ) -> bool {
        if !self.in_flight.insert(key.clone()) {
            return false;
        }
        let job = DecodeJob {
            key: key.clone(),
            bytes,
            style,
            dem_encoding,
        };
        let mut receipt = Receipt {
            key: Some(key),
            results: self.results_tx.clone(),
        };
        self.executor.spawn(Box::new(move || {
            let (key, decoded) = decode(job);
            // Ran: the receipt must not also report it lost.
            receipt.key = None;
            let _ = receipt.results.send(Outcome::Done(key, decoded));
        }));
        true
    }

    /// Apply ready results until `budget` is spent or none remain.
    /// `apply` uploads one decoded tile to the GPU caches.
    pub fn drain(&mut self, budget: Duration, mut apply: impl FnMut(Decoded)) {
        let start = Instant::now();
        while let Ok(outcome) = self.results.try_recv() {
            let (key, decoded) = match outcome {
                Outcome::Done(key, decoded) => (key, decoded),
                Outcome::Lost(key) => panic!(
                    "turbomap: the decode executor dropped the job for {} without running it, \
                     or its decode panicked; the tile would stay pending forever",
                    key.describe()
                ),
            };
            self.in_flight.remove(&key);
            if let Some(d) = decoded {
                apply(d);
            }
            if start.elapsed() >= budget {
                break;
            }
        }
    }

    /// Enqueued-but-unapplied count — non-zero must keep render-on-demand
    /// hosts awake (it is folded into `is_animating`). In-flight keys are
    /// also the accept→apply dedup window (`enqueue` refuses re-entry), so
    /// a tile is never decoded twice for one delivery.
    pub fn backlog(&self) -> usize {
        self.in_flight.len()
    }
}

// ---- wasm: no threads — decode inline, under the same budget -------------

#[cfg(target_arch = "wasm32")]
pub(crate) struct DecodeQueue {
    jobs: std::collections::VecDeque<DecodeJob>,
    in_flight: HashSet<QueueKey>,
}

#[cfg(target_arch = "wasm32")]
impl DecodeQueue {
    pub fn new() -> Self {
        Self {
            jobs: std::collections::VecDeque::new(),
            in_flight: HashSet::new(),
        }
    }

    pub fn enqueue(
        &mut self,
        key: QueueKey,
        bytes: Vec<u8>,
        style: Option<(Arc<VectorStyle>, u64)>,
        dem_encoding: Option<DemEncoding>,
    ) -> bool {
        if !self.in_flight.insert(key.clone()) {
            return false;
        }
        self.jobs.push_back(DecodeJob {
            key,
            bytes,
            style,
            dem_encoding,
        });
        true
    }

    /// Same interface as native, but the decode itself happens here — the
    /// budget bounds decode+apply together, time-slicing a burst across
    /// frames on the single web thread.
    pub fn drain(&mut self, budget: Duration, mut apply: impl FnMut(Decoded)) {
        let start = Instant::now();
        while let Some(job) = self.jobs.pop_front() {
            let (key, decoded) = decode(job);
            self.in_flight.remove(&key);
            if let Some(d) = decoded {
                apply(d);
            }
            if start.elapsed() >= budget {
                break;
            }
        }
    }

    pub fn backlog(&self) -> usize {
        self.in_flight.len()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn pool() -> Arc<dyn turbomap_core::work::Executor> {
        Arc::new(turbomap_core::work::ThreadPool::new(
            "decode-test",
            std::num::NonZeroUsize::new(2).unwrap(),
        ))
    }

    fn png_1x1() -> Vec<u8> {
        // Encode a real 1×1 PNG through the same crate that decodes it.
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn decodes_off_thread_and_applies_within_budget() {
        let mut q = DecodeQueue::new(pool());
        let key = QueueKey::Terrain {
            tile: TileId::new(3, 1, 2),
        };
        assert!(q.enqueue(key.clone(), png_1x1(), None, None));
        assert_eq!(q.backlog(), 1);
        // The same key is deduped while in flight.
        assert!(!q.enqueue(key, png_1x1(), None, None));

        // Terrain jobs run the DEM codec in the worker (plan D3): the apply
        // side receives real heights, not RGBA. Pixel (1,2,3) in Mapbox
        // Terrain-RGB is (1·65536 + 2·256 + 3)·0.1 − 10000 = −3394.9 m.
        let mut applied = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while applied.is_empty() && Instant::now() < deadline {
            q.drain(Duration::from_millis(4), |d| {
                if let DecodedKind::Dem { dem } = d.kind {
                    applied.push(dem);
                }
            });
        }
        assert_eq!(applied.len(), 1);
        let dem = &applied[0];
        assert_eq!((dem.width, dem.height), (1, 1));
        assert!(
            (dem.heights_m[0] - (-3394.9)).abs() < 0.05,
            "got {}",
            dem.heights_m[0]
        );
        assert_eq!(dem.coverage[0], 1);
        assert_eq!(q.backlog(), 0, "apply must clear the dedup entry");
    }

    #[test]
    fn a_decode_failure_clears_the_key_for_retry() {
        let mut q = DecodeQueue::new(pool());
        let key = QueueKey::Raster {
            layer_id: "base".into(),
            tile: TileId::new(1, 0, 0),
        };
        assert!(q.enqueue(key.clone(), b"not an image".to_vec(), None, None));
        let deadline = Instant::now() + Duration::from_secs(5);
        while q.backlog() > 0 && Instant::now() < deadline {
            q.drain(Duration::from_millis(4), |_| {
                panic!("garbage must not apply")
            });
        }
        assert_eq!(q.backlog(), 0);
        // Retriable: the key is free again.
        assert!(q.enqueue(key, png_1x1(), None, None));
    }

    #[test]
    fn vector_jobs_tessellate_off_thread_and_carry_their_epoch() {
        use turbomap_mvt::encode::TileEncoder;
        let bytes = TileEncoder::new()
            .layer("roads", 4096)
            .line(&[(0, 0), (4096, 4096)], &[])
            .finish()
            .finish();
        let mut q = DecodeQueue::new(pool());
        let key = QueueKey::Vector {
            layer_id: "roads-l".into(),
            tile: TileId::new(3, 1, 2),
        };
        // An empty style tessellates to an empty mesh — the point here is
        // the off-thread MVT decode + tessellate round-trip and the epoch
        // passthrough, not styling.
        assert!(q.enqueue(
            key,
            bytes,
            Some((Arc::new(VectorStyle::default()), 7)),
            None
        ));
        let mut got = None;
        let deadline = Instant::now() + Duration::from_secs(5);
        while got.is_none() && Instant::now() < deadline {
            q.drain(Duration::from_millis(4), |d| {
                if let DecodedKind::Vector { epoch, .. } = d.kind {
                    got = Some(epoch);
                }
            });
        }
        assert_eq!(got, Some(7), "the apply side needs the enqueue-time epoch");
        assert_eq!(q.backlog(), 0);
    }

    /// A host's own executor runs the decodes: here, one that runs each
    /// job on the calling thread, so the tile is decoded by the time
    /// `enqueue` returns and applies on the first drain.
    #[test]
    fn decodes_run_on_the_executor_the_host_supplies() {
        struct OnCaller(std::sync::atomic::AtomicUsize);
        impl turbomap_core::work::Executor for OnCaller {
            fn spawn(&self, job: turbomap_core::work::Job) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                job();
            }
        }
        let host = Arc::new(OnCaller(Default::default()));
        let mut q = DecodeQueue::new(host.clone());
        let key = QueueKey::Raster {
            layer_id: "base".into(),
            tile: TileId::new(1, 0, 0),
        };
        assert!(q.enqueue(key, png_1x1(), None, None));
        assert_eq!(host.0.load(std::sync::atomic::Ordering::SeqCst), 1);
        let mut applied = 0;
        q.drain(Duration::from_secs(1), |d| {
            if let DecodedKind::Image { rgba, w, h } = d.kind {
                assert_eq!((w, h, rgba.as_slice()), (1, 1, &[1u8, 2, 3, 255][..]));
                applied += 1;
            }
        });
        assert_eq!(
            applied, 1,
            "decoded on the host's executor, applied on the first drain"
        );
        assert_eq!(q.backlog(), 0);
    }

    /// An executor that loses a job (drops it unrun) is a bug the engine
    /// names on its next frame. Before, the tile stayed pending forever and
    /// a render-on-demand host spun waiting for it.
    #[test]
    #[should_panic(expected = "dropped the job for vector layer \"roads\" tile")]
    fn an_executor_that_loses_a_job_fails_naming_the_tile() {
        struct Discards;
        impl turbomap_core::work::Executor for Discards {
            fn spawn(&self, job: turbomap_core::work::Job) {
                drop(job);
            }
        }
        let mut q = DecodeQueue::new(Arc::new(Discards));
        let key = QueueKey::Vector {
            layer_id: "roads".into(),
            tile: TileId::new(3, 1, 2),
        };
        assert!(q.enqueue(
            key,
            Vec::new(),
            Some((Arc::new(VectorStyle::default()), 1)),
            None
        ));
        q.drain(Duration::from_secs(1), |_| panic!("nothing was decoded"));
    }
}
