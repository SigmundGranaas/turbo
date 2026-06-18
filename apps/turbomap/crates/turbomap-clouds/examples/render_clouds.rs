//! Offscreen demo + visual evaluation harness for the procedural cloud
//! overlay. No window, no network, no real GPU — renders through the same
//! headless Lavapipe readback the golden suite uses.
//!
//! It builds a synthetic but realistic radar sequence (a frontal rain band
//! with embedded convective cores), then scrubs a virtual time slider
//! across it — forward and back — crossfading between radar timesteps while
//! the procedural cloud detail keeps drifting. Artifacts written to the
//! output dir (default `/tmp/turbo-clouds`):
//!
//! - `frame_###.png` — every rendered step of the forward scrub.
//! - `contact_sheet.png` — a montage thumbnail strip of the scrub.
//! - `before_after.png` — raw blocky radar (left) vs. procedural clouds.
//! - `scrub.gif` — the time slider animating forward then backward.
//!
//! Run: `cargo run -p turbomap-clouds --example render_clouds [OUT_DIR]`

use std::fs::{self, File};
use std::path::Path;

use image::{imageops, GenericImage, Rgba, RgbaImage};
use turbomap_clouds::{CloudParams, CloudScene, RadarFrame, SyntheticStorm};
use turbomap_golden::gpu;

const WIDTH: u32 = 900;
const HEIGHT: u32 = 600;
/// Rendered steps between two consecutive radar timesteps (the crossfade
/// resolution of the time slider).
const SUBSTEPS: usize = 6;

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/turbo-clouds".to_string());
    fs::create_dir_all(&out_dir).expect("create out dir");

    let Some(gpu) = gpu::headless() else {
        eprintln!("no wgpu adapter available; cannot render");
        std::process::exit(1);
    };
    eprintln!("adapter: {}", gpu.adapter_name);

    let storm = SyntheticStorm::default();
    let frames = storm.generate();
    eprintln!(
        "generated {} radar frames at {}x{}",
        frames.len(),
        storm.width,
        storm.height
    );

    let scene = CloudScene::new(
        &gpu.device,
        &gpu.queue,
        gpu::TARGET_FORMAT,
        storm.width,
        storm.height,
    );

    // Fast tuning path: `CLOUDS_QUICK=1` renders a single mid-sequence
    // frame + the before/after so the look can be iterated in seconds.
    if std::env::var("CLOUDS_QUICK").is_ok() {
        write_before_after(&gpu, &scene, &frames, Path::new(&out_dir));
        eprintln!("quick -> {out_dir}/before_after.png");
        return;
    }

    // Walk a global timeline across all radar frames, crossfading each
    // adjacent pair over SUBSTEPS rendered steps.
    let total_steps = (frames.len() - 1) * SUBSTEPS + 1;
    let mut rendered: Vec<RgbaImage> = Vec::with_capacity(total_steps);

    for step in 0..total_steps {
        let idx = (step / SUBSTEPS).min(frames.len() - 2);
        let blend = (step % SUBSTEPS) as f32 / SUBSTEPS as f32;
        let blend = if step == total_steps - 1 { 1.0 } else { blend };

        scene.upload(&gpu.queue, 0, &frames[idx]);
        scene.upload(&gpu.queue, 1, &frames[idx + 1]);

        let params = CloudParams {
            resolution: [WIDTH as f32, HEIGHT as f32],
            // Animation clock advances continuously so clouds drift/boil
            // independently of which radar pair we are between.
            time: step as f32 * 0.55,
            blend,
            ..Default::default()
        };

        let img = gpu::render_to_image(&gpu, WIDTH, HEIGHT, |enc, view| {
            scene.render(&gpu.queue, enc, view, &params, true);
        });

        let path = format!("{out_dir}/frame_{step:03}.png");
        img.save(&path).expect("save frame");
        rendered.push(img);
        eprintln!(
            "rendered step {step}/{} (blend {blend:.2})",
            total_steps - 1
        );
    }

    write_contact_sheet(&rendered, Path::new(&out_dir));
    write_before_after(&gpu, &scene, &frames, Path::new(&out_dir));
    write_scrub_gif(&rendered, Path::new(&out_dir));

    eprintln!("done -> {out_dir}");
}

/// 3-column montage of evenly spaced steps, so the whole scrub reads at a
/// glance in one image.
fn write_contact_sheet(frames: &[RgbaImage], out_dir: &Path) {
    let picks: Vec<&RgbaImage> = (0..9)
        .map(|i| &frames[(i * (frames.len() - 1)) / 8])
        .collect();
    let cols = 3u32;
    let rows = 3u32;
    let tw = WIDTH / 3;
    let th = HEIGHT / 3;
    let pad = 6u32;
    let sheet_w = cols * tw + (cols + 1) * pad;
    let sheet_h = rows * th + (rows + 1) * pad;
    let mut sheet = RgbaImage::from_pixel(sheet_w, sheet_h, Rgba([22, 24, 28, 255]));
    for (i, img) in picks.iter().enumerate() {
        let thumb = imageops::resize(*img, tw, th, imageops::FilterType::Triangle);
        let c = i as u32 % cols;
        let r = i as u32 / cols;
        let x = pad + c * (tw + pad);
        let y = pad + r * (th + pad);
        sheet.copy_from(&thumb, x, y).expect("place thumb");
    }
    sheet
        .save(out_dir.join("contact_sheet.png"))
        .expect("save contact sheet");
}

/// Side-by-side: the raw blocky radar product (nearest-neighbour, classic
/// reflectivity palette) vs. the procedural cloud render of the same
/// timestep — the whole point of the feature in one frame.
fn write_before_after(gpu: &gpu::Gpu, scene: &CloudScene, frames: &[RadarFrame], out_dir: &Path) {
    let idx = frames.len() / 2;
    scene.upload(&gpu.queue, 0, &frames[idx]);
    scene.upload(&gpu.queue, 1, &frames[idx]);
    let params = CloudParams {
        resolution: [WIDTH as f32, HEIGHT as f32],
        time: idx as f32 * SUBSTEPS as f32 * 0.55,
        blend: 0.0,
        ..Default::default()
    };
    let after = gpu::render_to_image(gpu, WIDTH, HEIGHT, |enc, view| {
        scene.render(&gpu.queue, enc, view, &params, true);
    });
    let before = radar_to_blocky(&frames[idx], WIDTH, HEIGHT);

    let pad = 8u32;
    let mut combo = RgbaImage::from_pixel(
        WIDTH * 2 + pad * 3,
        HEIGHT + pad * 2,
        Rgba([22, 24, 28, 255]),
    );
    combo.copy_from(&before, pad, pad).expect("place before");
    combo
        .copy_from(&after, WIDTH + pad * 2, pad)
        .expect("place after");
    combo
        .save(out_dir.join("before_after.png"))
        .expect("save before_after");
}

/// Colourise a radar frame the "old" way: nearest-neighbour upscaling with
/// a discrete reflectivity ramp, so it looks like the blocky source.
fn radar_to_blocky(frame: &RadarFrame, w: u32, h: u32) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let cx = (x * frame.width / w).min(frame.width - 1);
            let cy = (y * frame.height / h).min(frame.height - 1);
            let cell = frame.cells[(cy * frame.width + cx) as usize];
            img.put_pixel(x, y, reflectivity_color(cell.precip, cell.coverage));
        }
    }
    img
}

/// Discrete radar reflectivity palette (transparent -> blue -> green ->
/// yellow -> red) over a flat grey sky, mimicking the blocky overlay.
fn reflectivity_color(precip: f32, coverage: f32) -> Rgba<u8> {
    let sky = [120u8, 130, 140];
    if precip < 0.04 {
        // Faint grey cloud shading from coverage only.
        let g = (coverage * 60.0) as u8;
        return Rgba([sky[0] - g / 2, sky[1] - g / 2, sky[2] - g / 2, 255]);
    }
    let stops = [
        (0.05f32, [80u8, 150, 230]),
        (0.25, [70, 200, 120]),
        (0.45, [240, 230, 80]),
        (0.65, [240, 150, 50]),
        (0.85, [220, 50, 50]),
    ];
    let mut col = stops[0].1;
    for (t, c) in stops {
        if precip >= t {
            col = c;
        }
    }
    Rgba([col[0], col[1], col[2], 255])
}

/// Encode the forward scrub followed by the reverse scrub as a looping
/// GIF — the time slider sweeping both directions.
fn write_scrub_gif(frames: &[RgbaImage], out_dir: &Path) {
    let gw = (WIDTH / 2) as u16;
    let gh = (HEIGHT / 2) as u16;
    let mut file = File::create(out_dir.join("scrub.gif")).expect("create gif");
    let mut encoder = gif::Encoder::new(&mut file, gw, gh, &[]).expect("gif encoder");
    encoder.set_repeat(gif::Repeat::Infinite).expect("repeat");

    let forward = frames.iter();
    let backward = frames.iter().rev().skip(1);
    for img in forward.chain(backward) {
        let small = imageops::resize(img, gw as u32, gh as u32, imageops::FilterType::Triangle);
        let mut data = small.into_raw();
        let mut gframe = gif::Frame::from_rgba_speed(gw, gh, &mut data, 10);
        gframe.delay = 9; // ~90 ms / frame
        encoder.write_frame(&gframe).expect("write gif frame");
    }
}
