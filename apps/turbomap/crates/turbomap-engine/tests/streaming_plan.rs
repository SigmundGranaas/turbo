//! The plan boundary's behavioural contract (slice B3.2): a plan-driven host
//! walks start → deliver / start → move away → cancel → acknowledge, and the
//! lifecycle table agrees with the legacy bookkeeping at every step. This is
//! the loop the FFI/web hosts adopt in B3.3.
#![cfg(feature = "gpu-tests")]

mod common;

use common::SyntheticResolver;
use turbomap_core::map::PendingTile;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{headless, Gpu, TARGET_FORMAT};
use turbomap_scene::{Layer, Paint, Scene, SourceDef};

fn raster_scene() -> Scene {
    let mut scene = Scene::new();
    scene.sources.insert(
        "base".to_string(),
        SourceDef::RasterXyz {
            tiles: vec!["https://example.test/{z}/{x}/{y}.png".to_string()],
            tile_size: 256,
            min_zoom: 0,
            max_zoom: 22,
            attribution: None,
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene
}

fn engine(gpu: &Gpu) -> TurbomapEngine {
    TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (512, 384),
        CameraState::new(LatLng::new(60.39, 5.32), 9.0),
        MapOptions {
            fade_in_secs: 0.0,
            ..Default::default()
        },
        Box::new(SyntheticResolver),
    )
    .expect("construct TurbomapEngine")
}

fn gpu_or_skip() -> Option<Gpu> {
    match headless() {
        Some(g) => Some(g),
        None => {
            if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("REQUIRE_GPU=1 but no wgpu adapter available");
            }
            eprintln!("SKIP: no wgpu adapter available");
            None
        }
    }
}

/// A 1×1 white PNG — enough for `ingest_raster_encoded`.
fn tiny_png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255]));
    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("encode png");
    bytes
}

#[test]
fn plan_start_deliver_cancel_acknowledge_loop_keeps_the_table_honest() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut e = engine(&gpu);
    e.apply(raster_scene());

    // 1) First plan: budget-truncated, priority-ordered starts, no cancels.
    let plan = e.streaming_plan(4);
    assert_eq!(plan.start.len(), 4, "budget truncates the start list");
    assert!(plan.cancel.is_empty(), "nothing in flight yet");
    e.lifecycle_agreement().expect("agreement after planning");

    // Re-planning immediately does NOT restart in-flight attempts.
    let replan = e.streaming_plan(64);
    let started: std::collections::HashSet<u64> =
        plan.start.iter().map(|r| r.id.0).collect();
    assert!(
        replan.start.iter().all(|r| !started.contains(&r.id.0)),
        "a live attempt must not be handed out twice"
    );

    // 2) Deliver one started fetch through the ordinary ingest path — the
    //    delivery completes the attempt (no RequestId plumbing needed yet).
    let png = tiny_png();
    let first = &plan.start[0];
    let PendingTile::Raster { layer_id, tile } = &first.fetch else {
        panic!("raster scene plans raster fetches");
    };
    assert!(e.ingest_raster_encoded(layer_id, *tile, &png));
    e.lifecycle_agreement().expect("agreement after delivery");

    // 3) Fail another started fetch: it must re-pend (appear in a later
    //    plan), not vanish.
    let second = plan.start[1].clone();
    e.fetch_failed(second.id);
    e.lifecycle_agreement().expect("agreement after failure");
    let tile_of = |p: &PendingTile| match p {
        PendingTile::Raster { tile, .. } => *tile,
        _ => panic!("raster scene"),
    };
    let failed_tile = tile_of(&second.fetch);
    let replan2 = e.streaming_plan(64);
    assert!(
        replan2.start.iter().any(|r| tile_of(&r.fetch) == failed_tile),
        "a failed still-wanted fetch re-pends on the next plan"
    );

    // 4) Move the camera far away: the next plan cancels the now-stale
    //    in-flight attempts; acknowledging them empties the cancel list.
    e.set_camera(CameraState::new(LatLng::new(45.0, -70.0), 9.0));
    let far_plan = e.streaming_plan(0);
    assert!(
        !far_plan.cancel.is_empty(),
        "attempts for the abandoned viewport must be cancelled"
    );
    e.lifecycle_agreement().expect("agreement with stale in-flight");
    for id in &far_plan.cancel {
        e.fetch_cancelled(*id);
    }
    let acked = e.streaming_plan(0);
    assert!(acked.cancel.is_empty(), "acknowledged cancels don't repeat");
    e.lifecycle_agreement().expect("agreement after acknowledgement");
}
