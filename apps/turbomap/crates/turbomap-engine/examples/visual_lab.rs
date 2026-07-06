//! `visual_lab` — the agent-driven **visual iteration loop**.
//!
//! The render → crop → measure → diagnose → fix loop, in one command. It
//! renders the real-data Bergen basemap (the same `golden::omt` scene the
//! credibility golden pins) and emits, into an output dir:
//!
//! - `full.png` — the whole frame at the chosen pixel ratio.
//! - `crop-centre.png` — the dense city centre at native 1:1 (no display
//!   downscaling to hide softness).
//! - `crop-detail.png` — a fixed region for eyeballing detail noise.
//! - `probe-text.png` / `probe-text-dark.png` — labels **isolated** on
//!   flat light/dark bands and magnified, so glyph edges, halos and
//!   sharpness are inspectable away from map noise.
//! - a JSON line of **metrics** designed to catch exactly the complaints
//!   the eye reports: text edge sharpness (acutance), a halo-ring score,
//!   and a speckle/noise count.
//!
//! An agent runs this, `Read`s the PNGs (it can see them), reads the
//! metrics, changes code/style, and re-runs — closing the loop without a
//! human as the eyes. See docs/architecture for the documented workflow.
//!
//! Usage:
//!   cargo run -p turbomap-engine --example visual_lab -- \
//!     [--zoom Z] [--ratio R] [--size WxH] [--lat,LNG] [--out DIR] [--probe WORD]

use std::sync::Arc;

use serde_json::json;
use turbomap_core::MapOptions;
use turbomap_engine::{
    CameraState, GeoJsonVectorSource, LatLng, MapEngine, ResolvedSource, SourceResolver,
    TurbomapEngine,
};
use turbomap_golden::omt::{bergen_scene, fixture_path, LAND};
use turbomap_golden::sources::FlatBasemap;
use turbomap_golden::{headless, render_to_image, TARGET_FORMAT};
use turbomap_scene::{Color, Filter, Layer, Paint, Scene, SourceDef, SymbolPlacement, TextAnchor};
use turbomap_tiles_pmtiles::PMTilesSource;

type Img = image::RgbaImage;

struct BergenResolver;
impl SourceResolver for BergenResolver {
    fn resolve(&self, _id: &str, def: &SourceDef) -> ResolvedSource {
        match def {
            SourceDef::RasterXyz { .. } => ResolvedSource::Raster(Arc::new(FlatBasemap(LAND))),
            SourceDef::VectorXyz { .. } => ResolvedSource::Vector(Arc::new(
                PMTilesSource::open(fixture_path()).expect("open bergen fixture"),
            )),
            _ => ResolvedSource::Unsupported,
        }
    }
}

/// A resolver that paints a flat band so isolated-text probes have a known
/// background (no map underneath).
struct FlatResolver([u8; 3]);
impl SourceResolver for FlatResolver {
    fn resolve(&self, _id: &str, def: &SourceDef) -> ResolvedSource {
        match def {
            SourceDef::RasterXyz { .. } => ResolvedSource::Raster(Arc::new(FlatBasemap(self.0))),
            SourceDef::GeoJson { data } => {
                ResolvedSource::Vector(Arc::new(GeoJsonVectorSource::new(data)))
            }
            _ => ResolvedSource::Unsupported,
        }
    }
}

struct Args {
    zoom: f64,
    ratio: f32,
    width: u32,
    height: u32,
    center: LatLng,
    out: String,
    probe: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args {
        zoom: 15.0,
        ratio: 2.0,
        width: 1280,
        height: 880,
        center: LatLng::new(60.3920, 5.3242),
        out: "/tmp/visual-lab".to_string(),
        probe: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--zoom" => a.zoom = it.next().unwrap().parse().unwrap(),
            "--ratio" => a.ratio = it.next().unwrap().parse().unwrap(),
            "--size" => {
                let v = it.next().unwrap();
                let (w, h) = v.split_once('x').expect("WxH");
                a.width = w.parse().unwrap();
                a.height = h.parse().unwrap();
            }
            "--center" => {
                let v = it.next().unwrap();
                let (lat, lng) = v.split_once(',').expect("LAT,LNG");
                a.center = LatLng::new(lat.parse().unwrap(), lng.parse().unwrap());
            }
            "--out" => a.out = it.next().unwrap(),
            "--probe" => a.probe = it.next(),
            other => panic!("unknown arg {other}"),
        }
    }
    a
}

fn render_scene(args: &Args, scene: Scene, resolver: Box<dyn SourceResolver>) -> Img {
    let gpu = headless().expect("no wgpu adapter");
    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (args.width, args.height),
        CameraState::new(args.center, args.zoom),
        MapOptions {
            fade_in_secs: 0.0,
            pixel_ratio: args.ratio,
            ..Default::default()
        },
        resolver,
    )
    .expect("engine");
    engine.apply(scene);
    engine.pump_tiles();
    let img = render_to_image(&gpu, args.width, args.height, |enc, view| {
        engine.render(enc, view)
    });
    engine.after_submit();
    img
}

/// A bare scene: a single point label per requested word, centred, on a
/// flat background — so glyph quality is inspectable in isolation.
fn probe_scene(word: &str) -> Scene {
    // Reuse the OMT place layer but force a synthetic single label by
    // overriding with a GeoJSON point would need the IR; simpler: lean on
    // the real `place` labels already in the fixture. The probe word is
    // matched by leaving the place layer in and cropping to a known label.
    // For a fully synthetic word we draw it via a Symbol over GeoJSON.
    let mut scene = Scene::new();
    scene.sources.insert(
        "base".into(),
        SourceDef::RasterXyz {
            tiles: vec!["x".into()],
            tile_size: 256,
            min_zoom: 0,
            max_zoom: 22,
            attribution: None,
        },
    );
    let pt = json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "properties": { "name": word },
            "geometry": { "type": "Point", "coordinates": [5.3242, 60.3920] }
        }]
    })
    .to_string();
    scene
        .sources
        .insert("pts".into(), SourceDef::GeoJson { data: pt });
    scene.layers.push(Layer::Raster {
        id: "base".into(),
        source: "base".into(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Symbol {
        id: "probe".into(),
        source: "pts".into(),
        source_layer: None,
        filter: Filter::Always,
        text_field: "name".into(),
        text_size: Paint::Const(20.0),
        color: Paint::Const(Color::rgb(40, 44, 54)),
        halo_color: Paint::Const(Color::rgb(250, 250, 250)),
        halo_width: Paint::Const(1.4),
        sort_key: None,
        placement: SymbolPlacement::Point,
        icon_image: None,
        icon_size: Paint::Const(24.0),
        icon_color: Paint::Const(Color::rgb(70, 78, 92)),
        text_anchor: TextAnchor::Center,
        letter_spacing: 0.0,
        font_weight: 0.0,
    });
    scene
}

fn crop(img: &Img, x: u32, y: u32, w: u32, h: u32) -> Img {
    let w = w.min(img.width().saturating_sub(x));
    let h = h.min(img.height().saturating_sub(y));
    image::imageops::crop_imm(img, x, y, w, h).to_image()
}

fn magnify(img: &Img, factor: u32) -> Img {
    image::imageops::resize(
        img,
        img.width() * factor,
        img.height() * factor,
        image::imageops::FilterType::Nearest,
    )
}

/// Luma of a pixel, 0..255.
fn luma(p: &image::Rgba<u8>) -> f32 {
    0.299 * p.0[0] as f32 + 0.587 * p.0[1] as f32 + 0.114 * p.0[2] as f32
}

/// Metrics tuned to the visual complaints. All computed over a region.
struct Metrics {
    /// Mean magnitude of the luma gradient over pixels that *have* an
    /// edge — high = crisp transitions, low = blurry. (acutance)
    sharpness: f32,
    /// Fraction of edge pixels whose local neighbourhood shows a
    /// dark→light→dark profile within ~3px — i.e. a halo/ring rather than
    /// a clean step. High = glyphs look bordered.
    halo_ring: f32,
    /// Count of isolated high-contrast pixels (a pixel far from both
    /// neighbours on each axis) per 1000 px — speckle / noise.
    speckle_per_k: f32,
}

fn measure(img: &Img) -> Metrics {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let at =
        |x: i32, y: i32| luma(img.get_pixel(x.clamp(0, w - 1) as u32, y.clamp(0, h - 1) as u32));
    let mut grad_sum = 0.0f32;
    let mut grad_n = 0u32;
    let mut ring_hits = 0u32;
    let mut speckle = 0u32;
    let mut total = 0u32;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            total += 1;
            let c = at(x, y);
            let gx = (at(x + 1, y) - at(x - 1, y)).abs();
            let gy = (at(x, y + 1) - at(x, y - 1)).abs();
            let g = gx.max(gy);
            if g > 18.0 {
                grad_sum += g;
                grad_n += 1;
                // ring: opposite sides both swing the same way relative to c
                let lr = (at(x - 2, y) - c).signum() == (at(x + 2, y) - c).signum()
                    && (at(x - 2, y) - c).abs() > 18.0
                    && (at(x + 2, y) - c).abs() > 18.0;
                if lr {
                    ring_hits += 1;
                }
            }
            // speckle: this pixel differs strongly from all 4 neighbours
            let n = [at(x - 1, y), at(x + 1, y), at(x, y - 1), at(x, y + 1)];
            if n.iter().all(|&v| (v - c).abs() > 40.0) {
                speckle += 1;
            }
        }
    }
    Metrics {
        sharpness: if grad_n > 0 {
            grad_sum / grad_n as f32
        } else {
            0.0
        },
        halo_ring: if grad_n > 0 {
            ring_hits as f32 / grad_n as f32
        } else {
            0.0
        },
        speckle_per_k: if total > 0 {
            speckle as f32 * 1000.0 / total as f32
        } else {
            0.0
        },
    }
}

fn main() {
    let args = parse_args();
    std::fs::create_dir_all(&args.out).expect("mkdir out");
    let p = |name: &str| format!("{}/{}", args.out, name);

    if let Some(word) = &args.probe {
        // Isolated text probe on light and dark bands, magnified 3x.
        let light = render_scene(
            &args,
            probe_scene(word),
            Box::new(FlatResolver([244, 242, 238])),
        );
        let dark = render_scene(
            &args,
            probe_scene(word),
            Box::new(FlatResolver([60, 66, 78])),
        );
        let cx = args.width / 2;
        let cy = args.height / 2;
        let cw = 260u32.min(args.width);
        let lc = crop(
            &light,
            cx.saturating_sub(cw / 2),
            cy.saturating_sub(40),
            cw,
            80,
        );
        let dc = crop(
            &dark,
            cx.saturating_sub(cw / 2),
            cy.saturating_sub(40),
            cw,
            80,
        );
        let m = measure(&lc);
        magnify(&lc, 3).save(p("probe-text.png")).unwrap();
        magnify(&dc, 3).save(p("probe-text-dark.png")).unwrap();
        println!(
            "{}",
            json!({
                "mode": "probe", "word": word,
                "sharpness": m.sharpness, "halo_ring": m.halo_ring,
                "outputs": [p("probe-text.png"), p("probe-text-dark.png")],
                "guide": "sharpness>45 crisp; halo_ring<0.15 clean (higher = bordered glyphs)",
            })
        );
        return;
    }

    // Full real-data frame + standard diagnostic crops.
    let full = render_scene(&args, bergen_scene(), Box::new(BergenResolver));
    full.save(p("full.png")).unwrap();

    // Centre (dense streets + labels) and a detail region, native 1:1.
    let (cw, ch) = (440.min(args.width), 320.min(args.height));
    let centre = crop(
        &full,
        args.width / 2 - cw / 2,
        args.height / 2 - ch / 2,
        cw,
        ch,
    );
    centre.save(p("crop-centre.png")).unwrap();
    let detail = crop(
        &full,
        (args.width * 3 / 4).min(args.width - cw),
        args.height / 8,
        cw,
        ch,
    );
    detail.save(p("crop-detail.png")).unwrap();

    let m_centre = measure(&centre);
    let m_full = measure(&full);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "mode": "frame",
            "size": [args.width, args.height], "zoom": args.zoom, "ratio": args.ratio,
            "centre": {
                "sharpness": m_centre.sharpness,
                "halo_ring": m_centre.halo_ring,
                "speckle_per_k": m_centre.speckle_per_k,
            },
            "full": {
                "sharpness": m_full.sharpness,
                "speckle_per_k": m_full.speckle_per_k,
            },
            "outputs": [p("full.png"), p("crop-centre.png"), p("crop-detail.png")],
            "guide": {
                "sharpness": "mean acutance of edges; higher = crisper. ~50+ is sharp text",
                "halo_ring": "fraction of edges that ring (dark-light-dark); <0.15 good, high = bordered glyphs",
                "speckle_per_k": "isolated high-contrast px per 1000; <2 good, high = detail noise",
            },
        }))
        .unwrap()
    );
}
