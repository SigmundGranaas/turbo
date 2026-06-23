//! Tile fetch pumps. Vector tiles flow through `VectorFetchPump` which
//! does HTTP fetch → MVT decode → tessellation off-thread. Raster tiles
//! (used for basemaps and DEM sources) flow through `RasterFetchPump`
//! which does HTTP fetch → PNG/WebP decode → RGBA bytes off-thread.

use std::sync::{Arc, Mutex};

use crossbeam_channel::{Receiver, Sender};
use turbomap_core::{
    tessellate, IconRequest, InteractiveFeature, LabelRequest, Mesh, TileId, TileSource,
    VectorStyle, VectorTileSource,
};

pub enum VectorOutcome {
    Decoded {
        id: TileId,
        mesh: Mesh,
        water_mesh: Mesh,
        labels: Vec<LabelRequest>,
        icons: Vec<IconRequest>,
        interactive: Vec<InteractiveFeature>,
    },
    Failed(TileId),
}

pub struct VectorFetchPump {
    pool: rayon::ThreadPool,
    tx: Sender<VectorOutcome>,
    pub rx: Receiver<VectorOutcome>,
    source: Arc<dyn VectorTileSource>,
    /// Each spawned job clones this Arc and tessellates against the
    /// snapshot. Hot-swapping the style updates future jobs but leaves
    /// in-flight ones alone (they'll be re-tessellated by the host if
    /// needed when the style change is committed).
    style: Arc<Mutex<Arc<VectorStyle>>>,
}

impl VectorFetchPump {
    pub fn new(
        source: Arc<dyn VectorTileSource>,
        style: VectorStyle,
        worker_threads: usize,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(worker_threads)
            .thread_name(|i| format!("turbomap-vector-fetch-{i}"))
            .build()
            .expect("rayon pool must build");
        Self {
            pool,
            tx,
            rx,
            source,
            style: Arc::new(Mutex::new(Arc::new(style))),
        }
    }

    /// Replace the style used by future spawned tessellations. Already-
    /// in-flight tiles are unaffected.
    #[allow(dead_code)] // staged for live style swap (see roadmap #4 in TODO)
    pub fn set_style(&self, style: VectorStyle) {
        *self.style.lock().expect("style mutex") = Arc::new(style);
    }

    pub fn spawn_fetch(&self, id: TileId) {
        let source = self.source.clone();
        let tx = self.tx.clone();
        let style_snapshot = self.style.lock().expect("style mutex").clone();
        self.pool.spawn(move || match source.request(id) {
            Ok(tile) => {
                let out = tessellate(id, &tile, &style_snapshot);
                let _ = tx.send(VectorOutcome::Decoded {
                    id,
                    mesh: out.mesh,
                    water_mesh: out.water_mesh,
                    labels: out.labels,
                    icons: out.icons,
                    interactive: out.interactive,
                });
            }
            Err(e) => {
                log::warn!("vector tile {id:?} fetch failed: {e}");
                let _ = tx.send(VectorOutcome::Failed(id));
            }
        });
    }
}

// ---- raster pump ---------------------------------------------------------

pub enum RasterOutcome {
    Decoded {
        id: TileId,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    Failed(TileId),
}

pub struct RasterFetchPump {
    pool: rayon::ThreadPool,
    tx: Sender<RasterOutcome>,
    pub rx: Receiver<RasterOutcome>,
    source: Arc<dyn TileSource>,
}

impl RasterFetchPump {
    pub fn new(source: Arc<dyn TileSource>, worker_threads: usize) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(worker_threads)
            .thread_name(|i| format!("turbomap-raster-fetch-{i}"))
            .build()
            .expect("rayon pool must build");
        Self {
            pool,
            tx,
            rx,
            source,
        }
    }

    pub fn spawn_fetch(&self, id: TileId) {
        let source = self.source.clone();
        let tx = self.tx.clone();
        self.pool.spawn(move || match source.request(id) {
            Ok(raw) => match image::load_from_memory(&raw.bytes) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let _ = tx.send(RasterOutcome::Decoded {
                        id,
                        rgba: rgba.into_raw(),
                        width: w,
                        height: h,
                    });
                }
                Err(e) => {
                    log::warn!("raster tile {id:?} decode failed: {e}");
                    let _ = tx.send(RasterOutcome::Failed(id));
                }
            },
            Err(e) => {
                log::warn!("raster tile {id:?} fetch failed: {e}");
                let _ = tx.send(RasterOutcome::Failed(id));
            }
        });
    }
}
