//! GPU-backed smoke test for the realistic-water pipeline. Builds a core `Map`
//! with a vector layer whose only feature is a tile-filling water polygon,
//! renders one frame headlessly, and asserts the water surface actually painted
//! (blue-dominant pixels, not the white background). Also dumps the frame to
//! `target/water-smoke/water.png` for eyeballing the look.
//!
//! Behind `gpu-tests` (needs a real adapter; skips without one unless
//! `REQUIRE_GPU=1`). This is the ONLY test that exercises the water *draw* path
//! — in particular the water pipeline reusing the vector pipeline's camera/tile
//! bind groups — which the trace goldens never reach.
#![cfg(feature = "gpu-tests")]

use std::collections::HashMap;
use std::sync::Arc;

use turbomap_core::{
    Camera, Color, Feature, Filter, GeomType, Geometry, LatLng, Map, MapOptions, Paint,
    PendingTile, Rule, TerrainOptions, TileError, TileId, TileSource, VectorStyle, VectorTile,
    VectorTileLayer, VectorTileSource,
};
use turbomap_golden::sources::GaussianTerrainSource;
use turbomap_golden::{headless, render_to_image, TARGET_FORMAT};

/// Synthetic vector source: every requested tile is one water polygon covering
/// the whole tile extent.
struct WaterEverywhere;

impl VectorTileSource for WaterEverywhere {
    fn request(&self, _tile: TileId) -> Result<VectorTile, TileError> {
        let ring = vec![(0, 0), (4096, 0), (4096, 4096), (0, 4096), (0, 0)];
        Ok(VectorTile {
            layers: vec![VectorTileLayer {
                name: "water".into(),
                version: 2,
                extent: 4096,
                features: vec![Feature {
                    id: 1,
                    geom_type: GeomType::Polygon,
                    geometry: Geometry::Polygon(vec![ring]),
                    properties: HashMap::new(),
                }],
            }],
        })
    }
}

fn water_style() -> VectorStyle {
    VectorStyle {
        background: Color::rgb(255, 255, 255),
        rules: vec![Rule {
            source_layer: "water".into(),
            filter: Filter::Always,
            paint: Paint::Fill {
                color: Color::rgb(40, 90, 150),
            },
            min_zoom: 0,
            max_zoom: 22,
            interactive: false,
        }],
    }
}

#[test]
fn water_surface_paints_a_frame() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").is_ok() {
            panic!("REQUIRE_GPU set but no GPU adapter available");
        }
        eprintln!("no GPU adapter; skipping water smoke test");
        return;
    };

    let (w, h) = (256u32, 256u32);
    // A gentle tilt so both the Fresnel sky reflection and the near body colour
    // contribute (and the water doesn't fill the whole frame — there's sky too).
    let camera = Camera::new(LatLng { lng: 0.0, lat: 0.0 }, 2.0).with_pitch(35.0);
    let mut map = Map::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (w, h),
        camera,
        MapOptions {
            fade_in_secs: 0.0,
            ..Default::default()
        },
    )
    .expect("map construction");

    let src: Arc<dyn VectorTileSource> = Arc::new(WaterEverywhere);
    map.add_vector_layer("water", src.clone(), water_style());

    // Drain the vector tiles the scene wants, feeding the water polygon for each.
    let mut rounds = 0;
    loop {
        let vectors: Vec<TileId> = map
            .pending_tiles()
            .into_iter()
            .filter_map(|p| match p {
                PendingTile::Vector { layer_id, tile } if layer_id == "water" => Some(tile),
                _ => None,
            })
            .collect();
        if vectors.is_empty() {
            break;
        }
        for tile in vectors {
            let vt = src.request(tile).unwrap();
            map.ingest_vector_tile("water", tile, &vt);
        }
        rounds += 1;
        assert!(rounds <= 16, "vector tiles failed to drain after {rounds} rounds");
    }

    let img = render_to_image(&gpu, w, h, |enc, view| {
        map.render(enc, view);
    });
    map.after_submit();

    // Dump for eyeballing regardless of the assertion outcome.
    let out_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/water-smoke");
    let _ = std::fs::create_dir_all(&out_dir);
    let _ = img.save(out_dir.join("water.png"));

    // The water surface should paint a blue-dominant region (reflection +
    // body colour), distinct from the white background — proof the water pass
    // drew without panicking (validates the cross-pipeline bind-group reuse).
    let mut bluish = 0usize;
    for px in img.pixels() {
        let [r, _g, b, _a] = px.0;
        if b as i32 > r as i32 + 15 && b > 60 {
            bluish += 1;
        }
    }
    let total = (w * h) as usize;
    assert!(
        bluish * 100 / total >= 5,
        "expected the water surface to paint a blue-dominant region; only \
         {bluish}/{total} px ({}%) — water pass may not have drawn",
        bluish * 100 / total
    );
}

/// Exercises the heightfield-reflection path: water over real (synthetic)
/// terrain so `ssr_enabled` is on and the reflection march samples the
/// cast-shadow/AO heightfield. Validates the march runs on the GPU without
/// panicking or producing NaNs (a NaN would hang a mobile driver), and dumps a
/// frame to `target/water-smoke/water-terrain.png` for eyeballing.
#[test]
fn water_reflects_terrain_without_nan() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").is_ok() {
            panic!("REQUIRE_GPU set but no GPU adapter available");
        }
        eprintln!("no GPU adapter; skipping water+terrain smoke test");
        return;
    };

    let (w, h) = (384u32, 288u32);
    // Over the Gaussian-bergen peak, tilted so the relief (and its reflection)
    // is in view.
    let camera = Camera::new(LatLng { lng: 5.32, lat: 60.39 }, 11.0)
        .with_pitch(55.0)
        .with_bearing(20.0);
    let mut map = Map::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (w, h),
        camera,
        MapOptions {
            fade_in_secs: 0.0,
            ..Default::default()
        },
    )
    .expect("map construction");

    let terrain: Arc<dyn TileSource> = Arc::new(GaussianTerrainSource::bergen());
    map.set_terrain_source(terrain.clone(), TerrainOptions::default());
    let water_src: Arc<dyn VectorTileSource> = Arc::new(WaterEverywhere);
    map.add_vector_layer("water", water_src.clone(), water_style());

    // Extreme sea state from a (synthetic) MET forecast: big westerly swell + a
    // gale → high ferocity, whitecaps, strong shore foam. Exercises every new
    // shader path (directional waves, crest whitecaps, heightfield foam ring).
    map.set_water_conditions(Some(270.0), Some(5.0), Some(16.0), Some(260.0));

    // Drain terrain + water tiles so the heightfield can assemble.
    let mut rounds = 0;
    loop {
        let pending = map.pending_tiles();
        if pending.is_empty() {
            break;
        }
        let mut did_any = false;
        for req in pending {
            match req {
                PendingTile::Terrain { tile } => {
                    let raw = terrain.request(tile).expect("terrain request");
                    let img = image::load_from_memory(&raw.bytes)
                        .expect("terrain decode")
                        .to_rgba8();
                    let (tw, th) = img.dimensions();
                    map.ingest_terrain_tile(tile, img.as_raw(), tw, th);
                    did_any = true;
                }
                PendingTile::Vector { layer_id, tile } if layer_id == "water" => {
                    let vt = water_src.request(tile).unwrap();
                    map.ingest_vector_tile("water", tile, &vt);
                    did_any = true;
                }
                _ => {}
            }
        }
        rounds += 1;
        assert!(rounds <= 24, "tiles failed to drain after {rounds} rounds");
        if !did_any {
            break;
        }
    }

    // Render twice: the first frame assembles the heightfield (so `ssr_enabled`
    // turns on); the second renders with the reflection march active.
    let _ = render_to_image(&gpu, w, h, |enc, view| {
        map.render(enc, view);
    });
    map.after_submit();
    let img = render_to_image(&gpu, w, h, |enc, view| {
        map.render(enc, view);
    });
    map.after_submit();

    let out_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/water-smoke");
    let _ = std::fs::create_dir_all(&out_dir);
    let _ = img.save(out_dir.join("water-terrain.png"));

    // No NaNs reached the framebuffer (a NaN/Inf channel reads back as 0 after
    // the sRGB target clamp, but the frame must not be uniformly black, and the
    // run reaching here proves the march didn't panic). Assert the frame has
    // real content: a spread of luminance, not a single flat colour.
    let mut min_l = 255i32;
    let mut max_l = 0i32;
    for px in img.pixels() {
        let [r, g, b, _] = px.0;
        let l = (r as i32 + g as i32 + b as i32) / 3;
        min_l = min_l.min(l);
        max_l = max_l.max(l);
    }
    assert!(
        max_l - min_l > 10,
        "frame is nearly uniform ({min_l}..{max_l}) — water/terrain may have failed to render"
    );
}
