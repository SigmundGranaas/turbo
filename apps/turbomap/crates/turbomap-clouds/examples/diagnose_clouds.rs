//! Cloud-fidelity diagnostic harness.
//!
//! Where `render_clouds` is a *demo* (pretty time-scrub artifacts), this is
//! an *instrument*: it decomposes the shader into its internal stages and
//! measures how faithfully the rendered overlay represents the radar data
//! it was given. Use it to find *why* the clouds look wrong and to tune the
//! look against numbers, not vibes.
//!
//! Renders headless through the same Lavapipe readback the golden suite
//! uses. Artifacts (default `/tmp/turbo-clouds-diag`):
//!
//! - `debug_channels.png` — every pipeline stage (radar precip/coverage →
//!   cloud field → density → light → alpha → albedo → final) side by side
//!   for one frame. The decomposition that shows where the look comes from.
//! - `aov_*.png` — each of those stages as its own full-size image.
//! - `param_sweep.png` — the final look across a `map_scale × softness`
//!   grid, to see what the knobs actually do.
//! - `fidelity.txt` — the scorecard (also printed to stdout).
//!
//! Run: `cargo run -p turbomap-clouds --example diagnose_clouds [OUT_DIR]`

use std::fs;
use std::path::Path;

use glam::{Mat3, Mat4, Vec3};
use image::{imageops, GenericImage, Rgba, RgbaImage};
use turbomap_clouds::metrics::{self, Fidelity};
use turbomap_clouds::{CloudParams, CloudScene, DebugView, RadarFrame, SyntheticStorm};
use turbomap_golden::gpu;

const WIDTH: u32 = 720;
const HEIGHT: u32 = 480;

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/turbo-clouds-diag".to_string());
    fs::create_dir_all(&out_dir).expect("create out dir");
    let out = Path::new(&out_dir);

    // Fast CPU check of the per-pixel ray math (no GPU): print the parallax
    // shift at a few sample pixels so a broken matrix/scale is obvious.
    if std::env::var("CAM_DEBUG").is_ok() {
        let aspect = WIDTH as f32 / HEIGHT as f32;
        let (alt_top, w2uv) = (0.26f32, 1.5f32);
        for pitch_deg in [0.0f32, 35.0, 55.0] {
            let inv = glam::Mat4::from_cols_array_2d(&pitched_inv_view_proj(pitch_deg, aspect));
            eprintln!("--- pitch {pitch_deg}° ---");
            for (ux, uy) in [(0.5f32, 0.5f32), (0.5, 0.1), (0.5, 0.9), (0.1, 0.5)] {
                let ndc = glam::vec2(ux * 2.0 - 1.0, 1.0 - 2.0 * uy);
                let pn = inv * glam::vec4(ndc.x, ndc.y, 0.0, 1.0);
                let pf = inv * glam::vec4(ndc.x, ndc.y, 1.0, 1.0);
                let ro = pn.truncate() / pn.w;
                let rd = (pf.truncate() / pf.w - ro).normalize();
                let g = ro + rd * (-ro.z / rd.z);
                let pt = ro + rd * ((alt_top - ro.z) / rd.z);
                let shift = (pt.truncate() - g.truncate()) * w2uv;
                eprintln!(
                    "  uv=({ux},{uy}) ro=({:.2},{:.2},{:.2}) rd=({:.2},{:.2},{:.2}) g=({:.2},{:.2}) shift=({:.3},{:.3})",
                    ro.x, ro.y, ro.z, rd.x, rd.y, rd.z, g.x, g.y, shift.x, shift.y
                );
            }
        }
        return;
    }

    let Some(gpu) = gpu::headless() else {
        eprintln!("no wgpu adapter available; cannot render");
        std::process::exit(1);
    };
    eprintln!("adapter: {}", gpu.adapter_name);

    let storm = SyntheticStorm::default();
    let frames = storm.generate();
    let scene = CloudScene::new(&gpu.device, gpu::TARGET_FORMAT, storm.width, storm.height);

    // A representative mid-sequence frame: the front is crossing the grid
    // with convective cores embedded — the case the look has to nail.
    let idx = frames.len() / 2;
    scene.upload(&gpu.queue, 0, &frames[idx]);
    scene.upload(&gpu.queue, 1, &frames[idx]);
    let base = CloudParams {
        resolution: [WIDTH as f32, HEIGHT as f32],
        time: idx as f32 * 3.0,
        blend: 0.0,
        ..Default::default()
    };

    // --- 1. AOV decomposition --------------------------------------------
    let mut labelled: Vec<(&str, RgbaImage)> = Vec::new();
    for view in DebugView::ALL {
        let params = CloudParams {
            debug_view: view,
            ..base
        };
        let img = render(&gpu, &scene, &params);
        img.save(out.join(format!("aov_{}.png", view.label().replace(' ', "_"))))
            .expect("save aov");
        labelled.push((view.label(), img));
    }
    save_montage(&labelled, 4, out.join("debug_channels.png"));
    eprintln!(
        "decomposition -> {out_dir}/debug_channels.png (order: {})",
        DebugView::ALL
            .iter()
            .map(|v| v.label())
            .collect::<Vec<_>>()
            .join(" | ")
    );

    // --- 2. Fidelity scorecard -------------------------------------------
    let report = score_sequence(&gpu, &scene, &frames, storm.width, storm.height);
    println!("\n{report}");
    fs::write(out.join("fidelity.txt"), &report).expect("write fidelity.txt");

    // --- 3. Parameter sweep ----------------------------------------------
    let scales = [2.0f32, 4.0, 6.0, 9.0];
    let softs = [0.2f32, 0.5, 0.85];
    let mut sweep: Vec<(&str, RgbaImage)> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    for &soft in &softs {
        for &scale in &scales {
            let params = CloudParams {
                map_scale: scale,
                softness: soft,
                ..base
            };
            sweep.push(("", render(&gpu, &scene, &params)));
            labels.push(format!("scale={scale} soft={soft}"));
        }
    }
    let sweep_ref: Vec<(&str, RgbaImage)> = sweep
        .iter()
        .enumerate()
        .map(|(i, (_, img))| (labels[i].as_str(), img.clone()))
        .collect();
    save_montage(&sweep_ref, scales.len() as u32, out.join("param_sweep.png"));
    eprintln!(
        "param sweep -> {out_dir}/param_sweep.png (rows: soft {:?}, cols: scale {:?})",
        softs, scales
    );

    // --- 4. Lighting sweep: sun elevation × extinction --------------------
    let elevs = [0.12f32, 0.28, 0.5, 0.8];
    let exts = [5.0f32, 9.0, 14.0];
    let mut lsweep: Vec<RgbaImage> = Vec::new();
    let mut llabels: Vec<String> = Vec::new();
    for &ext in &exts {
        for &el in &elevs {
            let params = CloudParams {
                sun_elevation: el,
                extinction: ext,
                light_extinction: ext + 3.0,
                ..base
            };
            lsweep.push(render(&gpu, &scene, &params));
            llabels.push(format!("el={el} ext={ext}"));
        }
    }
    let lref: Vec<(&str, RgbaImage)> = lsweep
        .iter()
        .enumerate()
        .map(|(i, img)| (llabels[i].as_str(), img.clone()))
        .collect();
    save_montage(&lref, elevs.len() as u32, out.join("lighting_sweep.png"));
    eprintln!(
        "lighting sweep -> {out_dir}/lighting_sweep.png (rows: ext {:?}, cols: sun-elev {:?})",
        exts, elevs
    );

    // --- 5. Tilted "hero": the view raked through the slab (map pitched) --
    // parallax 0 is top-down (flat); a nonzero value reveals the puff sides.
    for (tag, px) in [("flat", 0.0f32), ("tilted", 0.35)] {
        let params = CloudParams {
            parallax: px,
            ..base
        };
        render(&gpu, &scene, &params)
            .save(out.join(format!("hero_{tag}.png")))
            .expect("save hero");
    }
    eprintln!("hero -> {out_dir}/hero_flat.png + hero_tilted.png (parallax 0 vs 0.35)");

    // --- 6. Camera-ray heroes: the REAL per-pixel ray (map pitch) --------
    // Build a representative pitched perspective camera (same look_at_lh /
    // perspective_lh / +y-south convention as turbomap-core) and feed its
    // inverse view-projection so the shader reconstructs each pixel's ray and
    // rakes through the cloud slab — this is the production path validated.
    let aspect = WIDTH as f32 / HEIGHT as f32;
    for pitch_deg in [0.0f32, 35.0, 55.0] {
        let inv_vp = pitched_inv_view_proj(pitch_deg, aspect);
        let params = CloudParams {
            use_camera_ray: true,
            inv_view_proj: inv_vp,
            cloud_alt_base: 0.10,
            cloud_alt_top: 0.26,
            world_to_uv: 1.5,
            ..base
        };
        render(&gpu, &scene, &params)
            .save(out.join(format!("hero_cam_{}deg.png", pitch_deg as u32)))
            .expect("save cam hero");
    }
    eprintln!("camera-ray heroes -> {out_dir}/hero_cam_{{0,35,55}}deg.png (real pitched ray)");

    // Isolation: use_camera_ray ON but world_to_uv=0 → shift forced to zero.
    // Should be identical to flat top-down. If clouds vanish here, the bug is
    // the use_ray FLAG path, not the shift; if clouds appear, it's the shift.
    for pitch_deg in [0.0f32, 55.0] {
        let params = CloudParams {
            use_camera_ray: true,
            inv_view_proj: pitched_inv_view_proj(pitch_deg, aspect),
            cloud_alt_base: 0.10,
            cloud_alt_top: 0.26,
            world_to_uv: 1.5,
            debug_view: DebugView::Parallax,
            ..base
        };
        render(&gpu, &scene, &params)
            .save(out.join(format!("parallax_dbg_{}deg.png", pitch_deg as u32)))
            .expect("save");
    }
    eprintln!(
        "parallax debug -> {out_dir}/parallax_dbg_{{0,55}}deg.png (R=shift.x G=shift.y grey=0)"
    );

    eprintln!("done -> {out_dir}");
}

/// Inverse view-projection for a camera over the origin, pitched back by
/// `pitch_deg`, looking north — mirroring turbomap-core's `view_projection`
/// (left-handed look-at + perspective, world +y = south, +z = up). Lets the
/// harness drive the shader's real per-pixel ray path offscreen.
fn pitched_inv_view_proj(pitch_deg: f32, aspect: f32) -> [[f32; 4]; 4] {
    let fov_y = 0.6435f32; // matches core FOV_Y
    let altitude = 1.0f32;
    let pitch = pitch_deg.to_radians();
    // Eye: (0,0,alt) pitched about X by -pitch (eye swings to +y/south), target at origin.
    let eye = Mat3::from_rotation_x(-pitch) * Vec3::new(0.0, 0.0, altitude);
    let up = Vec3::new(0.0, -1.0, 0.0); // world-north
    let view = Mat4::look_at_lh(eye, Vec3::ZERO, up);
    let proj = Mat4::perspective_lh(fov_y, aspect, altitude * 0.01, altitude * 100.0);
    (proj * view).inverse().to_cols_array_2d()
}

/// Render one frame with the given params (basemap cleared underneath, so
/// the target is always initialised; opaque AOVs overwrite it).
fn render(gpu: &gpu::Gpu, scene: &CloudScene, params: &CloudParams) -> RgbaImage {
    gpu::render_to_image(gpu, WIDTH, HEIGHT, |enc, view| {
        scene.render(&gpu.queue, enc, view, params, true);
    })
}

/// Compute the fidelity scorecard for every frame in the sequence and
/// format a human-readable report (per-frame rows + a sequence average).
fn score_sequence(
    gpu: &gpu::Gpu,
    scene: &CloudScene,
    frames: &[RadarFrame],
    gw: u32,
    gh: u32,
) -> String {
    let mut rows = String::new();
    let mut acc = [0.0f64; 6];
    for (fi, frame) in frames.iter().enumerate() {
        scene.upload(&gpu.queue, 0, frame);
        scene.upload(&gpu.queue, 1, frame);
        let base = CloudParams {
            resolution: [WIDTH as f32, HEIGHT as f32],
            time: fi as f32 * 3.0,
            blend: 0.0,
            ..Default::default()
        };

        let alpha_img = render(
            gpu,
            scene,
            &CloudParams {
                debug_view: DebugView::Alpha,
                ..base
            },
        );
        let albedo_img = render(
            gpu,
            scene,
            &CloudParams {
                debug_view: DebugView::Albedo,
                ..base
            },
        );

        let alpha = metrics::box_downsample(alpha_img.as_raw(), WIDTH, HEIGHT, gw, gh, 0);
        let lr = metrics::box_downsample(albedo_img.as_raw(), WIDTH, HEIGHT, gw, gh, 0);
        let lg = metrics::box_downsample(albedo_img.as_raw(), WIDTH, HEIGHT, gw, gh, 1);
        let lb = metrics::box_downsample(albedo_img.as_raw(), WIDTH, HEIGHT, gw, gh, 2);
        let luma: Vec<f32> = (0..lr.len())
            .map(|i| metrics::luminance(lr[i], lg[i], lb[i]))
            .collect();

        let coverage: Vec<f32> = frame.cells.iter().map(|c| c.coverage).collect();
        let precip: Vec<f32> = frame.cells.iter().map(|c| c.precip).collect();
        let f = metrics::evaluate(&coverage, &precip, &alpha, &luma);

        acc[0] += f.coverage_alpha_corr as f64;
        acc[1] += f.leak as f64;
        acc[2] += f.miss as f64;
        acc[3] += f.silhouette_iou as f64;
        acc[4] += f.precip_darkness_corr as f64;
        acc[5] += f.clouded_fraction as f64;
        rows.push_str(&format!("  frame {fi:2}  {}\n", fmt_row(&f)));
    }
    let n = frames.len() as f64;
    let avg = Fidelity {
        coverage_alpha_corr: (acc[0] / n) as f32,
        leak: (acc[1] / n) as f32,
        miss: (acc[2] / n) as f32,
        silhouette_iou: (acc[3] / n) as f32,
        precip_darkness_corr: (acc[4] / n) as f32,
        clouded_fraction: (acc[5] / n) as f32,
    };
    format!(
        "CLOUD FIDELITY  ({} frames, grid {gw}x{gh})\n\
         columns: cov→alpha corr (↑) | leak (↓) | miss (↓) | silhouette IoU (↑) | precip→dark corr (↑) | clouded frac\n\
         {rows}  ─────────\n  AVERAGE   {}\n",
        frames.len(),
        fmt_row(&avg),
    )
}

fn fmt_row(f: &Fidelity) -> String {
    format!(
        "corr {:+.2}  leak {:.2}  miss {:.2}  iou {:.2}  dark {:+.2}  cloud {:.2}",
        f.coverage_alpha_corr,
        f.leak,
        f.miss,
        f.silhouette_iou,
        f.precip_darkness_corr,
        f.clouded_fraction,
    )
}

/// Montage of labelled tiles, `cols` per row, with a caption bar painted
/// under each tile. Tile size is derived from the first image.
fn save_montage(tiles: &[(&str, RgbaImage)], cols: u32, path: std::path::PathBuf) {
    if tiles.is_empty() {
        return;
    }
    let tw = WIDTH / 3;
    let th = HEIGHT / 3;
    let cap = 14u32; // caption strip height
    let pad = 6u32;
    let rows = tiles.len().div_ceil(cols as usize) as u32;
    let cell_h = th + cap;
    let sheet_w = cols * tw + (cols + 1) * pad;
    let sheet_h = rows * cell_h + (rows + 1) * pad;
    let mut sheet = RgbaImage::from_pixel(sheet_w, sheet_h, Rgba([18, 20, 24, 255]));
    for (i, (label, img)) in tiles.iter().enumerate() {
        let thumb = imageops::resize(img, tw, th, imageops::FilterType::Triangle);
        let c = i as u32 % cols;
        let r = i as u32 / cols;
        let x = pad + c * (tw + pad);
        let y = pad + r * (cell_h + pad);
        sheet.copy_from(&thumb, x, y).expect("place thumb");
        draw_text(&mut sheet, x + 2, y + th + 3, label, 2);
    }
    sheet.save(path).expect("save montage");
}

// --- Minimal 3x5 bitmap font (uppercase) for self-documenting montages ---
// Each glyph is 5 rows of a 3-bit pattern (bit 2 = leftmost column).

fn draw_text(img: &mut RgbaImage, x0: u32, y0: u32, text: &str, scale: u32) {
    let mut cx = x0;
    for ch in text.to_ascii_uppercase().chars() {
        let glyph = glyph(ch);
        for (ry, row) in glyph.iter().enumerate() {
            for col in 0..3u32 {
                if row & (1 << (2 - col)) != 0 {
                    fill(img, cx + col * scale, y0 + ry as u32 * scale, scale);
                }
            }
        }
        cx += 4 * scale; // 3px glyph + 1px gap
    }
}

fn fill(img: &mut RgbaImage, x: u32, y: u32, s: u32) {
    for dy in 0..s {
        for dx in 0..s {
            let (px, py) = (x + dx, y + dy);
            if px < img.width() && py < img.height() {
                img.put_pixel(px, py, Rgba([235, 238, 245, 255]));
            }
        }
    }
}

/// 3x5 glyph rows for the characters used in our captions. Unknown chars
/// render blank.
fn glyph(c: char) -> [u8; 5] {
    match c {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b110, 0b101, 0b010],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b010, 0b101, 0b010, 0b101, 0b010],
        '9' => [0b010, 0b101, 0b011, 0b001, 0b110],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '=' => [0b000, 0b111, 0b000, 0b111, 0b000],
        _ => [0; 5], // space + unknown
    }
}
