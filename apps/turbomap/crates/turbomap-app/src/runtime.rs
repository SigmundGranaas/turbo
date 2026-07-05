//! Tile fetch pump: HTTP fetch off-thread, RAW BYTES back to the render
//! thread. That is the whole job (plan P6.2 / invariant 10): the engine's
//! codec owns image decode, DEM decode, and MVT tessellation — a host pump
//! that decoded or tessellated would put wire-format knowledge downstream
//! of the interpretation plane. The pre-P6.2 pumps here did exactly that
//! (PNG→RGBA and MVT→mesh host-side); both are gone.

use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use turbomap_core::TileId;

/// One finished fetch attempt, echoing the `(layer, tile)` it was spawned
/// for. `bytes: None` means the fetch failed (already logged).
pub struct FetchOutcome {
    pub layer: String,
    pub tile: TileId,
    pub bytes: Option<Vec<u8>>,
}

/// The fetch function a pump runs per tile: raw encoded bytes as served
/// (PNG/JPEG/WebP for rasters and DEMs, protobuf for MVT), or an error
/// string for the log.
pub type FetchFn = dyn Fn(TileId) -> Result<Vec<u8>, String> + Send + Sync;

/// A worker pool that turns `(layer, tile)` requests into [`FetchOutcome`]s
/// on a channel the render thread drains. Content-kind-agnostic: the fetch
/// closure captures whatever HTTP source (and disk cache) serves the bytes.
pub struct BytesPump {
    pool: rayon::ThreadPool,
    tx: Sender<FetchOutcome>,
    pub rx: Receiver<FetchOutcome>,
    fetch: Arc<FetchFn>,
    /// For log lines only ("vector tile … fetch failed").
    kind: &'static str,
}

impl BytesPump {
    pub fn new(
        kind: &'static str,
        worker_threads: usize,
        fetch: impl Fn(TileId) -> Result<Vec<u8>, String> + Send + Sync + 'static,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(worker_threads)
            .thread_name(move |i| format!("turbomap-{kind}-fetch-{i}"))
            .build()
            .expect("rayon pool must build");
        Self {
            pool,
            tx,
            rx,
            fetch: Arc::new(fetch),
            kind,
        }
    }

    pub fn spawn_fetch(&self, layer: String, tile: TileId) {
        let fetch = self.fetch.clone();
        let tx = self.tx.clone();
        let kind = self.kind;
        self.pool.spawn(move || {
            let bytes = match fetch(tile) {
                Ok(bytes) => Some(bytes),
                Err(e) => {
                    log::warn!("{kind} tile {tile:?} fetch failed: {e}");
                    None
                }
            };
            let _ = tx.send(FetchOutcome { layer, tile, bytes });
        });
    }
}
