//! FFI round-trip: this test plays the role of a foreign host (Kotlin/
//! Swift) and drives the map *only* through the uniffi-exported surface —
//! scene JSON in, pull/push tile IO, camera/projection/hit-test, and the
//! PNG snapshot out. If this passes, the generated bindings expose a
//! complete, working control plane.
#![cfg(feature = "gpu-tests")]

use std::io::Cursor;

use image::{ImageEncoder, RgbaImage};
use turbomap_ffi::{Camera, GeoPoint, Point, TileKind, TurboMap};

fn camera() -> Camera {
    Camera {
        lat: 60.39,
        lng: 5.32,
        zoom: 9.0,
        pitch_deg: 0.0,
        bearing_deg: 0.0,
    }
}

fn scene_json() -> String {
    r##"{
        "sources": {
            "base": { "type": "raster-xyz", "tiles": ["https://example.test/{z}/{x}/{y}.png"] },
            "route": { "type": "geo-json",
                "data": "{\"type\":\"LineString\",\"coordinates\":[[5.10,60.30],[5.32,60.39],[5.55,60.48]]}" }
        },
        "layers": [
            { "type": "raster", "id": "basemap", "source": "base" },
            { "type": "line", "id": "route", "source": "route",
              "color": { "const": { "r": 220, "g": 30, "b": 60, "a": 255 } },
              "width": { "const": 5.0 } }
        ]
    }"##
    .to_string()
}

/// A solid sea-green "fetched tile", encoded as a PNG like a server would.
fn fake_tile_png() -> Vec<u8> {
    let mut img = RgbaImage::new(256, 256);
    for px in img.pixels_mut() {
        *px = image::Rgba([90, 170, 140, 255]);
    }
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(Cursor::new(&mut out))
        .write_image(img.as_raw(), 256, 256, image::ExtendedColorType::Rgba8)
        .expect("encode");
    out
}

fn new_map() -> Option<TurboMap> {
    match TurboMap::headless(512, 384, camera()) {
        Ok(map) => Some(map),
        Err(e) => {
            if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("REQUIRE_GPU=1 but headless map failed: {e}");
            }
            eprintln!("SKIP: {e}");
            None
        }
    }
}

#[test]
fn full_host_roundtrip_through_the_ffi_surface() {
    let Some(map) = new_map() else { return };

    // 1. Apply the scene; both layers + sources arrive.
    let delta = map.apply_scene(scene_json()).expect("apply scene");
    assert_eq!(delta.layers_added, 2, "{delta:?}");
    assert_eq!(delta.sources_changed, 2, "{delta:?}");
    assert!(map.unsupported_layers().is_empty());

    // 2. GeoJSON drains in-process; remote raster tiles stay pending.
    let local = map.pump_local_tiles();
    assert!(local.vector_tiles > 0, "geojson should drain: {local:?}");
    let pending = map.pending_tiles();
    assert!(!pending.is_empty(), "remote raster tiles should be pending");
    assert!(pending.iter().all(|t| t.kind == TileKind::Raster));
    assert!(pending.iter().all(|t| t.layer_id.as_deref() == Some("basemap")));

    // 3. Host fetch loop: push an encoded PNG for every pending tile.
    let tile_bytes = fake_tile_png();
    for req in &pending {
        assert!(
            map.ingest_raster_tile(
                req.layer_id.clone().unwrap(),
                req.z,
                req.x,
                req.y,
                tile_bytes.clone()
            ),
            "tile {req:?} should decode"
        );
    }
    assert!(
        map.pending_tiles().is_empty(),
        "all pending tiles were ingested"
    );

    // 4. Snapshot through the FFI and check actual pixels: the sea-green
    //    basemap everywhere, the red route somewhere.
    let png = map.render_png().expect("render png");
    let img = image::load_from_memory(&png).expect("decode png").to_rgba8();
    assert_eq!((img.width(), img.height()), (512, 384));
    let centre = img.get_pixel(256, 192);
    let total = img.pixels().count();
    let greenish = img
        .pixels()
        .filter(|p| p.0[1] > p.0[0] && p.0[1] > 120 && p.0[2] > 100)
        .count();
    let reddish = img
        .pixels()
        .filter(|p| p.0[0] > 180 && p.0[1] < 100)
        .count();
    assert!(
        greenish * 2 > total,
        "basemap should dominate; centre={centre:?} greenish={greenish}/{total}"
    );
    assert!(reddish > 50, "route should be visible, reddish={reddish}");
}

#[test]
fn camera_projection_and_errors_through_the_ffi_surface() {
    let Some(map) = new_map() else { return };

    // Camera round-trip.
    let cam = map.camera();
    assert!((cam.lat - 60.39).abs() < 1e-9 && (cam.zoom - 9.0).abs() < 1e-9);
    map.set_camera(Camera { zoom: 11.0, ..cam });
    assert!((map.camera().zoom - 11.0).abs() < 1e-9);

    // project ∘ unproject ≈ identity at pitch 0.
    let geo = GeoPoint {
        lat: 60.40,
        lng: 5.33,
    };
    let screen = map.project(geo).expect("project");
    let back = map.unproject(screen).expect("unproject");
    assert!(
        (back.lat - geo.lat).abs() < 1e-6 && (back.lng - geo.lng).abs() < 1e-6,
        "round-trip drifted: {geo:?} -> {screen:?} -> {back:?}"
    );

    // Camera animation drives through tick().
    map.ease_to(
        Camera {
            zoom: 12.0,
            ..map.camera()
        },
        50,
    );
    let mut frames = 0;
    while map.tick() && frames < 1000 {
        frames += 1;
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        (map.camera().zoom - 12.0).abs() < 1e-6,
        "animation should land on target, got {}",
        map.camera().zoom
    );

    // Hit-testing a circle layer through the FFI.
    let scene = r##"{
        "sources": { "pts": { "type": "geo-json",
            "data": "{\"type\":\"Point\",\"coordinates\":[5.33,60.40]}" } },
        "layers": [ { "type": "circle", "id": "dot", "source": "pts",
            "color": { "const": { "r": 255, "g": 200, "b": 0, "a": 255 } },
            "radius": { "const": 10.0 } } ]
    }"##;
    map.apply_scene(scene.to_string()).expect("circle scene");
    let dot_screen = map.project(geo).expect("project dot");
    let hits = map.hit_test(
        Point {
            x: dot_screen.x,
            y: dot_screen.y,
        },
        8.0,
    );
    assert!(
        hits.iter().any(|h| h.layer_id == "dot"),
        "circle should be hit at its own position, got {hits:?}"
    );

    // Structured errors for bad scenes.
    assert!(map.apply_scene("not json".to_string()).is_err());
    let dangling = r#"{ "layers": [ { "type": "raster", "id": "x", "source": "missing" } ] }"#;
    let err = map.apply_scene(dangling.to_string()).unwrap_err();
    assert!(
        err.to_string().contains("unknown source"),
        "validation should name the problem, got: {err}"
    );
}
