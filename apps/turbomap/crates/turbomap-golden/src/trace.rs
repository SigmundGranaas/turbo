//! Record/replay trace format.
//!
//! A `Trace` is a self-contained, serialisable description of a map
//! scene — viewport, camera, and an ordered layer stack over named
//! synthetic sources. Replaying one is a pure function `Trace -> image`,
//! which is exactly what the golden suite needs: add a `.json` trace +
//! a reference `.png` and you have a new gated render test, no Rust
//! changes required.
//!
//! Phase 0 sources are synthetic (deterministic, offline). The format is
//! intentionally open for extension: a later phase can add a
//! `Tiles { dir }` source that replays *recorded* real tiles captured
//! from a live session, making real-world traces first-class fixtures.

use std::collections::HashMap;
use std::sync::Arc;

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use turbomap_core::{
    Camera, HillshadeStyle, LatLng, Map, MapOptions, PendingTile, TerrainOptions, TileSource,
};

use crate::gpu::{render_to_image, Gpu, TARGET_FORMAT};
use crate::sources::{GaussianTerrainSource, ParchmentBasemap};

/// Which deterministic synthetic source backs a layer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceSpec {
    /// Uniform parchment raster.
    Parchment,
    /// Gaussian Terrain-RGB DEM peaked on Bergen.
    GaussianBergen,
}

impl SourceSpec {
    fn build(self) -> Arc<dyn TileSource> {
        match self {
            SourceSpec::Parchment => Arc::new(ParchmentBasemap),
            SourceSpec::GaussianBergen => Arc::new(GaussianTerrainSource::bergen()),
        }
    }
}

/// One entry in the ordered, bottom-to-top layer stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LayerSpec {
    Raster { id: String, source: SourceSpec },
    Hillshade { id: String, terrain: SourceSpec },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraSpec {
    pub lat: f64,
    pub lng: f64,
    pub zoom: f64,
    #[serde(default)]
    pub pitch: f64,
    #[serde(default)]
    pub bearing: f64,
}

/// A complete, replayable scene description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub camera: CameraSpec,
    pub layers: Vec<LayerSpec>,
}

impl Trace {
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialize trace")
    }
}

/// Replay a trace to a final composite image. Drives the host pull/push
/// loop synchronously against the synthetic sources, then renders once.
pub fn replay(trace: &Trace, gpu: &Gpu) -> RgbaImage {
    let camera = Camera::new(
        LatLng {
            lng: trace.camera.lng,
            lat: trace.camera.lat,
        },
        trace.camera.zoom,
    )
    .with_pitch(trace.camera.pitch)
    .with_bearing(trace.camera.bearing);

    let mut map = Map::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (trace.width, trace.height),
        camera,
        MapOptions {
            // No fade-in: capture the final composite on the first frame.
            fade_in_secs: 0.0,
            ..Default::default()
        },
    )
    .expect("map construction");

    // Raster sources, keyed by layer id. The DEM is shared map-level
    // state (a hillshade layer draws from the single terrain source), so
    // it is tracked separately — `PendingTile::Terrain` carries no id.
    let mut raster_sources: HashMap<String, Arc<dyn TileSource>> = HashMap::new();
    let mut terrain_source: Option<Arc<dyn TileSource>> = None;

    for layer in &trace.layers {
        match layer {
            LayerSpec::Raster { id, source } => {
                let src = source.build();
                map.add_raster_layer(id, src.clone());
                raster_sources.insert(id.clone(), src);
            }
            LayerSpec::Hillshade { id, terrain } => {
                let src = terrain.build();
                map.set_terrain_source(src.clone(), TerrainOptions::default());
                map.add_hillshade_layer(id, HillshadeStyle::default());
                terrain_source = Some(src);
            }
        }
    }

    drain_pending(&mut map, &raster_sources, terrain_source.as_ref());
    let image = render_to_image(gpu, trace.width, trace.height, |enc, view| {
        map.render(enc, view)
    });
    map.after_submit();
    image
}

fn ingest_raster_tile(
    map: &mut Map,
    src: &Arc<dyn TileSource>,
    layer_id: &str,
    tile: turbomap_core::TileId,
) {
    let raw = src.request(tile).expect("raster request");
    let img = image::load_from_memory(&raw.bytes)
        .expect("raster decode")
        .to_rgba8();
    let (w, h) = img.dimensions();
    map.ingest_raster(layer_id, tile, img.as_raw(), w, h);
}

fn drain_pending(
    map: &mut Map,
    raster: &HashMap<String, Arc<dyn TileSource>>,
    terrain: Option<&Arc<dyn TileSource>>,
) {
    let mut rounds = 0;
    loop {
        let pending = map.pending_tiles();
        if pending.is_empty() {
            break;
        }
        for req in pending {
            match req {
                PendingTile::Raster { layer_id, tile } => {
                    if let Some(src) = raster.get(&layer_id) {
                        ingest_raster_tile(map, src, &layer_id, tile);
                    }
                }
                PendingTile::Terrain { tile } => {
                    if let Some(src) = terrain {
                        let raw = src.request(tile).expect("terrain request");
                        let img = image::load_from_memory(&raw.bytes)
                            .expect("terrain decode")
                            .to_rgba8();
                        let (w, h) = img.dimensions();
                        // The DEM codec runs at ingest (plan D3): raw
                        // Terrain-RGB → real heights + coverage.
                        let dem =
                            turbomap_core::decode_dem_rgba(img.as_raw(), w, h, src.dem_encoding())
                                .expect("dem decode");
                        map.ingest_terrain_tile(tile, &dem);
                    }
                }
                // No vector layers in Phase 0 traces. Hillshade no longer
                // emits its own pending tiles — the terrain source above
                // feeds the shared DEM the hillshade pass reads from.
                PendingTile::Vector { .. } | PendingTile::Hillshade { .. } => {}
            }
        }
        rounds += 1;
        assert!(
            rounds <= 16,
            "pending_tiles failed to drain after {rounds} rounds — \
             synthetic sources never fail, so this is a scene-state bug"
        );
    }
}
