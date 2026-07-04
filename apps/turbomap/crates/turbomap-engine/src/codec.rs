//! The decode queue — image decode OFF the render thread (plan B4.1).
//!
//! Before this, `ingest_raster_encoded`/`ingest_terrain_encoded` ran
//! `image::load_from_memory` on the calling thread — the render thread on
//! Android (mitigated by time-slicing the ingest drain), the main thread on
//! web. Now `ingest_*` only *accepts bytes*: decode runs on a small worker
//! pool (native) or inline under the apply budget (wasm has no threads),
//! and the decoded RGBA is applied to the GPU caches at the top of
//! `render()`, bounded by [`APPLY_BUDGET`] per frame.
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

use turbomap_core::{TileId, VectorStyle};
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
}

/// A decoded, ready-to-apply tile.
pub(crate) struct Decoded {
    pub key: QueueKey,
    pub kind: DecodedKind,
}

pub(crate) enum DecodedKind {
    /// Raster/DEM RGBA ready for GPU upload.
    Image { rgba: Vec<u8>, w: u32, h: u32 },
    /// A tessellated vector tile + the style epoch it was built against.
    Vector { out: turbomap_core::tessellate::TessellationOutput, epoch: u64 },
}

fn decode(job: DecodeJob) -> (QueueKey, Option<Decoded>) {
    let DecodeJob { key, bytes, style } = job;
    match &key {
        QueueKey::Raster { .. } | QueueKey::Terrain { .. } => {
            match image::load_from_memory(&bytes) {
                Ok(img) => {
                    let img = img.to_rgba8();
                    let (w, h) = img.dimensions();
                    let kind = DecodedKind::Image { rgba: img.into_raw(), w, h };
                    (key.clone(), Some(Decoded { key, kind }))
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

// ---- native: a small worker pool ----------------------------------------

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct DecodeQueue {
    jobs: crossbeam_channel::Sender<DecodeJob>,
    results: crossbeam_channel::Receiver<(QueueKey, Option<Decoded>)>,
    /// Keys enqueued and not yet applied/failed — the dedup set. Only the
    /// engine's thread touches it (`&mut self` API), so no lock.
    in_flight: HashSet<QueueKey>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DecodeQueue {
    pub fn new() -> Self {
        let (jobs, job_rx) = crossbeam_channel::unbounded::<DecodeJob>();
        let (result_tx, results) = crossbeam_channel::unbounded();
        // Two workers: image decode is the only work, and tile bursts are
        // bounded by the host's inflight caps — more threads would just
        // trade cache locality for no wall-clock win on mobile cores.
        for i in 0..2 {
            let rx = job_rx.clone();
            let tx = result_tx.clone();
            std::thread::Builder::new()
                .name(format!("turbomap-decode-{i}"))
                .spawn(move || {
                    while let Ok(job) = rx.recv() {
                        // A closed results channel means the engine dropped;
                        // exit quietly with it.
                        if tx.send(decode(job)).is_err() {
                            break;
                        }
                    }
                })
                .expect("spawn decode worker");
        }
        Self { jobs, results, in_flight: HashSet::new() }
    }

    /// Accept bytes for decode. Returns `false` (and drops the bytes) if
    /// this key is already in flight — the dedup that keeps a host's
    /// reconcile loop from re-decoding every not-yet-applied tile.
    pub fn enqueue(
        &mut self,
        key: QueueKey,
        bytes: Vec<u8>,
        style: Option<(Arc<VectorStyle>, u64)>,
    ) -> bool {
        if !self.in_flight.insert(key.clone()) {
            return false;
        }
        // Send can only fail if workers died (poisoned process state);
        // clear the dedup entry so the tile can be retried.
        if self.jobs.send(DecodeJob { key: key.clone(), bytes, style }).is_err() {
            self.in_flight.remove(&key);
            return false;
        }
        true
    }

    /// Apply ready results until `budget` is spent or none remain.
    /// `apply` uploads one decoded tile to the GPU caches.
    pub fn drain(&mut self, budget: Duration, mut apply: impl FnMut(Decoded)) {
        let start = Instant::now();
        while let Ok((key, decoded)) = self.results.try_recv() {
            self.in_flight.remove(&key);
            if let Some(d) = decoded {
                apply(d);
            }
            if start.elapsed() >= budget {
                break;
            }
        }
    }

    /// Whether `key` is currently in the accept→apply window. The engine
    /// subtracts these from `pending_tiles` so pull-driven hosts don't
    /// refetch every tile once per decode latency (the echo loop the sim's
    /// heavy-roaming gate caught).
    pub fn contains(&self, key: &QueueKey) -> bool {
        self.in_flight.contains(key)
    }

    /// Enqueued-but-unapplied count — non-zero must keep render-on-demand
    /// hosts awake (it is folded into `is_animating`).
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
        Self { jobs: std::collections::VecDeque::new(), in_flight: HashSet::new() }
    }

    pub fn enqueue(
        &mut self,
        key: QueueKey,
        bytes: Vec<u8>,
        style: Option<(Arc<VectorStyle>, u64)>,
    ) -> bool {
        if !self.in_flight.insert(key.clone()) {
            return false;
        }
        self.jobs.push_back(DecodeJob { key, bytes, style });
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

    /// See the native impl.
    pub fn contains(&self, key: &QueueKey) -> bool {
        self.in_flight.contains(key)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn png_1x1() -> Vec<u8> {
        // Encode a real 1×1 PNG through the same crate that decodes it.
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn decodes_off_thread_and_applies_within_budget() {
        let mut q = DecodeQueue::new();
        let key = QueueKey::Terrain { tile: TileId::new(3, 1, 2) };
        assert!(q.enqueue(key.clone(), png_1x1(), None));
        assert_eq!(q.backlog(), 1);
        // The same key is deduped while in flight.
        assert!(!q.enqueue(key, png_1x1(), None));

        let mut applied = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while applied.is_empty() && Instant::now() < deadline {
            q.drain(Duration::from_millis(4), |d| {
                if let DecodedKind::Image { rgba, w, h } = d.kind {
                    applied.push((w, h, rgba));
                }
            });
        }
        assert_eq!(applied, vec![(1, 1, vec![1, 2, 3, 255])]);
        assert_eq!(q.backlog(), 0, "apply must clear the dedup entry");
    }

    #[test]
    fn a_decode_failure_clears_the_key_for_retry() {
        let mut q = DecodeQueue::new();
        let key = QueueKey::Raster { layer_id: "base".into(), tile: TileId::new(1, 0, 0) };
        assert!(q.enqueue(key.clone(), b"not an image".to_vec(), None));
        let deadline = Instant::now() + Duration::from_secs(5);
        while q.backlog() > 0 && Instant::now() < deadline {
            q.drain(Duration::from_millis(4), |_| panic!("garbage must not apply"));
        }
        assert_eq!(q.backlog(), 0);
        // Retriable: the key is free again.
        assert!(q.enqueue(key, png_1x1(), None));
    }

    #[test]
    fn vector_jobs_tessellate_off_thread_and_carry_their_epoch() {
        use turbomap_mvt::encode::TileEncoder;
        let bytes = TileEncoder::new()
            .layer("roads", 4096)
            .line(&[(0, 0), (4096, 4096)], &[])
            .finish()
            .finish();
        let mut q = DecodeQueue::new();
        let key = QueueKey::Vector { layer_id: "roads-l".into(), tile: TileId::new(3, 1, 2) };
        // An empty style tessellates to an empty mesh — the point here is
        // the off-thread MVT decode + tessellate round-trip and the epoch
        // passthrough, not styling.
        assert!(q.enqueue(key, bytes, Some((Arc::new(VectorStyle::default()), 7))));
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
}
