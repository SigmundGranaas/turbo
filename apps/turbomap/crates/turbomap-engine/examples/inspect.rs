//! `inspect` — the agent-first development tool for the engine.
//!
//! It runs a `Scene` through the real `TurbomapEngine` headless and emits
//! a single machine-readable JSON report covering *every stage* of the
//! pipeline — scene + validation, the applied diff, which layers the
//! backend supports, tile-drain activity, per-layer render metrics + cache
//! stats, and projection round-trips — alongside the rendered PNG. The
//! point is that an agent (or a human) can inspect and reason about the
//! whole process from one artifact instead of guessing from pass/fail.
//!
//! Usage:
//!   cargo run -p turbomap-engine --example inspect -- \
//!     [--scene scene.json] [--prev prev.json] \
//!     [--center LAT,LNG] [--zoom Z] [--pitch DEG] [--bearing DEG] \
//!     [--size WxH] [--png out.png] [--report out.json]
//!
//! With no --scene it inspects a built-in raster+hillshade scene.

use std::sync::Arc;

use serde_json::{json, Value};
use turbomap_core::MapOptions;
use turbomap_engine::{
    CameraState, GeoJsonVectorSource, MapEngine, ResolvedSource, SceneDelta, SourceResolver,
    TurbomapEngine,
};
use turbomap_golden::sources::{GaussianTerrainSource, ParchmentBasemap};
use turbomap_golden::{headless, render_to_image, TARGET_FORMAT};
use turbomap_scene::diff::{LayerChange, SourceChange};
use turbomap_scene::{DemEncoding, Layer, LatLng, Paint, Scene, ScreenPoint, SourceDef};

struct SyntheticResolver;
impl SourceResolver for SyntheticResolver {
    fn resolve(&self, _id: &str, def: &SourceDef) -> ResolvedSource {
        match def {
            SourceDef::RasterXyz { .. } => ResolvedSource::Raster(Arc::new(ParchmentBasemap)),
            SourceDef::DemXyz { .. } => ResolvedSource::Dem(Arc::new(GaussianTerrainSource::bergen())),
            SourceDef::GeoJson { data } => {
                ResolvedSource::Vector(Arc::new(GeoJsonVectorSource::new(data)))
            }
            SourceDef::VectorXyz { .. }
            | SourceDef::PmtilesRaster { .. }
            | SourceDef::PmtilesVector { .. }
            | SourceDef::PmtilesDem { .. }
            | SourceDef::Chain { .. } => ResolvedSource::Unsupported,
        }
    }
}

struct Args {
    scene: Option<String>,
    prev: Option<String>,
    center: LatLng,
    zoom: f64,
    pitch: f64,
    bearing: f64,
    width: u32,
    height: u32,
    png: String,
    report: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args {
        scene: None,
        prev: None,
        center: LatLng::new(60.39, 5.32),
        zoom: 9.0,
        pitch: 0.0,
        bearing: 0.0,
        width: 512,
        height: 384,
        png: "/tmp/turbomap-inspect.png".to_string(),
        report: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--scene" => a.scene = it.next(),
            "--prev" => a.prev = it.next(),
            "--center" => {
                let v = it.next().expect("--center LAT,LNG");
                let (lat, lng) = v.split_once(',').expect("LAT,LNG");
                a.center = LatLng::new(lat.parse().unwrap(), lng.parse().unwrap());
            }
            "--zoom" => a.zoom = it.next().unwrap().parse().unwrap(),
            "--pitch" => a.pitch = it.next().unwrap().parse().unwrap(),
            "--bearing" => a.bearing = it.next().unwrap().parse().unwrap(),
            "--size" => {
                let v = it.next().expect("--size WxH");
                let (w, h) = v.split_once('x').expect("WxH");
                a.width = w.parse().unwrap();
                a.height = h.parse().unwrap();
            }
            "--png" => a.png = it.next().unwrap(),
            "--report" => a.report = it.next(),
            other => panic!("unknown arg: {other}"),
        }
    }
    a
}

fn default_scene() -> Scene {
    let mut s = Scene::new();
    s.sources.insert(
        "base".into(),
        SourceDef::RasterXyz {
            tiles: vec!["https://example.test/{z}/{x}/{y}.png".into()],
            tile_size: 256,
            min_zoom: 0,
            max_zoom: 22,
            attribution: Some("synthetic".into()),
        },
    );
    s.sources.insert(
        "dem".into(),
        SourceDef::DemXyz {
            tiles: vec!["https://example.test/dem/{z}/{x}/{y}.png".into()],
            encoding: DemEncoding::MapboxRgb,
            min_zoom: 0,
            max_zoom: 22,
            halo: 0,
        },
    );
    s.layers.push(Layer::Raster {
        id: "basemap".into(),
        source: "base".into(),
        opacity: Paint::Const(1.0),
    });
    s.layers.push(Layer::Hillshade {
        id: "hillshade".into(),
        source: "dem".into(),
        exaggeration: 1.5,
        height_only: false,
    });
    s
}

fn load_scene(path: &str) -> Scene {
    let json = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("parse {path}: {e}"))
}

fn source_type(def: &SourceDef) -> &'static str {
    match def {
        SourceDef::RasterXyz { .. } => "raster-xyz",
        SourceDef::VectorXyz { .. } => "vector-xyz",
        SourceDef::GeoJson { .. } => "geojson",
        SourceDef::DemXyz { .. } => "dem-xyz",
        SourceDef::PmtilesRaster { .. } => "pmtiles-raster",
        SourceDef::PmtilesVector { .. } => "pmtiles-vector",
        SourceDef::PmtilesDem { .. } => "pmtiles-dem",
        SourceDef::Chain { .. } => "chain",
    }
}

fn layer_type(layer: &Layer) -> &'static str {
    match layer {
        Layer::Raster { .. } => "raster",
        Layer::Fill { .. } => "fill",
        Layer::FillExtrusion { .. } => "fill-extrusion",
        Layer::Line { .. } => "line",
        Layer::Circle { .. } => "circle",
        Layer::Symbol { .. } => "symbol",
        Layer::Hillshade { .. } => "hillshade",
        Layer::Custom { .. } => "custom",
    }
}

fn scene_json(scene: &Scene) -> Value {
    let sources: Vec<Value> = scene
        .sources
        .iter()
        .map(|(id, def)| json!({ "id": id, "type": source_type(def) }))
        .collect();
    let layers: Vec<Value> = scene
        .layers
        .iter()
        .map(|l| json!({ "id": l.id(), "type": layer_type(l), "source": l.source() }))
        .collect();
    let valid = match scene.validate() {
        Ok(()) => json!(true),
        Err(e) => json!(e.to_string()),
    };
    json!({ "sources": sources, "layers": layers, "valid": valid })
}

fn delta_json(delta: &SceneDelta) -> Value {
    let sources: Vec<String> = delta
        .sources
        .iter()
        .map(|c| match c {
            SourceChange::Added(id) => format!("added {id}"),
            SourceChange::Removed(id) => format!("removed {id}"),
            SourceChange::Updated(id) => format!("updated {id}"),
        })
        .collect();
    let layers: Vec<String> = delta
        .layers
        .iter()
        .map(|c| match c {
            LayerChange::Added { id, index } => format!("added {id} @ {index}"),
            LayerChange::Removed { id } => format!("removed {id}"),
            LayerChange::Updated { id } => format!("updated {id}"),
            LayerChange::Moved { id, from, to } => format!("moved {id} {from}->{to}"),
        })
        .collect();
    json!({ "sources": sources, "layers": layers })
}

fn main() {
    let args = parse_args();
    let scene = args.scene.as_deref().map(load_scene).unwrap_or_else(default_scene);

    let gpu = headless().expect("no wgpu adapter (install mesa-vulkan-drivers for a software one)");

    let camera = CameraState {
        center: args.center,
        zoom: args.zoom,
        pitch_deg: args.pitch,
        bearing_deg: args.bearing,
    };
    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (args.width, args.height),
        camera,
        MapOptions {
            fade_in_secs: 0.0,
            ..Default::default()
        },
        Box::new(SyntheticResolver),
    )
    .expect("construct engine");

    // Optional baseline so the reported delta reflects a real edit.
    if let Some(prev) = args.prev.as_deref() {
        engine.apply(load_scene(prev));
        engine.pump_tiles();
    }

    let delta = engine.apply(scene.clone());
    let drain = engine.pump_tiles();

    let image = render_to_image(&gpu, args.width, args.height, |enc, view| engine.render(enc, view));
    engine.after_submit();
    image.save(&args.png).expect("write png");

    // Per-layer render metrics + cache stats.
    let metrics = engine.last_frame_metrics();
    let layer_metrics: Vec<Value> = metrics
        .layers
        .iter()
        .map(|lm| {
            json!({
                "id": lm.id,
                "kind": format!("{:?}", lm.kind),
                "cache": {
                    "entries": lm.cache.entries,
                    "bytes_used": lm.cache.bytes_used,
                    "hits": lm.cache.hits,
                    "misses": lm.cache.misses,
                },
            })
        })
        .collect();

    // Projection round-trips around the camera centre.
    let samples: Vec<Value> = [
        (args.center.lat, args.center.lng),
        (args.center.lat + 0.05, args.center.lng + 0.05),
        (args.center.lat - 0.05, args.center.lng - 0.05),
    ]
    .iter()
    .map(|&(lat, lng)| {
        let ll = LatLng::new(lat, lng);
        let screen = engine.project(ll);
        let roundtrip_ok = screen
            .and_then(|s| engine.unproject(s))
            .map(|b| (b.lat - lat).abs() < 1e-6 && (b.lng - lng).abs() < 1e-6)
            .unwrap_or(false);
        json!({
            "lat": lat, "lng": lng,
            "screen": screen.map(|s| vec![s.x, s.y]),
            "roundtrip_ok": roundtrip_ok,
        })
    })
    .collect();

    let centre_screen = ScreenPoint::new(args.width as f64 / 2.0, args.height as f64 / 2.0);
    let hits: Vec<Value> = engine
        .hit_test(centre_screen, 6.0)
        .iter()
        .map(|h| json!({ "layer_id": h.layer_id, "feature_id": h.feature_id }))
        .collect();

    let caps = engine.capabilities();
    let report = json!({
        "input": {
            "scene_path": args.scene,
            "size": [args.width, args.height],
            "camera": {
                "lat": args.center.lat, "lng": args.center.lng,
                "zoom": args.zoom, "pitch": args.pitch, "bearing": args.bearing,
            },
        },
        "scene": scene_json(&scene),
        "delta_from_prev": delta_json(&delta),
        "capabilities": {
            "custom_layers": caps.custom_layers,
            "terrain": caps.terrain,
            "data_driven_paint": caps.data_driven_paint,
            "max_texture_size": caps.max_texture_size,
        },
        "unsupported_layers": engine.unsupported_layers(),
        "tiles": {
            "drain_rounds": drain.rounds,
            "raster_tiles": drain.raster_tiles,
            "terrain_tiles": drain.terrain_tiles,
            "vector_tiles": drain.vector_tiles,
        },
        "render": {
            "adapter": gpu.adapter_name,
            "cpu_ms": metrics.cpu_time.as_secs_f64() * 1000.0,
            "gpu_ms": metrics.gpu_time.map(|d| d.as_secs_f64() * 1000.0),
            "layer_count": metrics.layer_count,
            "marker_count": metrics.marker_count,
            "layers": layer_metrics,
        },
        "projection_samples": samples,
        "hit_test_centre": hits,
        "outputs": { "png": args.png },
    });

    let pretty = serde_json::to_string_pretty(&report).unwrap();
    if let Some(path) = args.report.as_deref() {
        std::fs::write(path, &pretty).expect("write report");
    }
    println!("{pretty}");
}
