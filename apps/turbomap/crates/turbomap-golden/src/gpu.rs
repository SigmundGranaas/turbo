//! Headless wgpu device + deterministic offscreen readback.
//!
//! Golden tests need *reproducible* pixels, so we prefer a software
//! adapter (e.g. Lavapipe) — its output is deterministic for a given
//! Mesa version, unlike a real GPU which varies by vendor/driver. The
//! render + `copy_texture_to_buffer` happen in a single encoder so the
//! readback captures exactly the frame we rendered (the same proven
//! shape as `turbomap-app/examples/snapshot.rs`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use image::RgbaImage;

/// A headless GPU context. `None` from [`headless`] means no adapter is
/// available — callers should skip rather than fail.
pub struct Gpu {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    /// Human-readable adapter name, surfaced in test logs so a golden
    /// mismatch can be attributed to a driver change.
    pub adapter_name: String,
}

/// The colour-correct surface format the live demo renders through.
/// Golden references are captured in this format.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Build a headless context on the software adapter (or on hardware when
/// `TURBOMAP_GOLDEN_ADAPTER=hardware` asks for it). Returns `None` if that
/// adapter is not available — never a different one in its place.
pub fn headless() -> Option<Gpu> {
    let instance = wgpu::Instance::new({
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        desc.backends = wgpu::Backends::PRIMARY | wgpu::Backends::GL;
        desc
    });

    // The goldens are held on the software adapter (Lavapipe) — CI's, and
    // deterministic across machines. A hardware adapter is used only when
    // asked for by name (`TURBOMAP_GOLDEN_ADAPTER=hardware`), and the adapter
    // is printed with every golden. This used to fall back to any adapter
    // silently, so a box without Lavapipe certified a different rasteriser
    // against goldens tuned for another.
    let hardware = match std::env::var("TURBOMAP_GOLDEN_ADAPTER").as_deref() {
        Ok("hardware") => true,
        Ok("software") | Err(_) => false,
        Ok(other) => panic!(
            "TURBOMAP_GOLDEN_ADAPTER={other}: expected `software` (the default) or `hardware`"
        ),
    };
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: !hardware,
        apply_limit_buckets: false,
    }))
    .ok()?;

    let adapter_name = adapter.get_info().name;

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("turbomap-golden-device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        memory_hints: wgpu::MemoryHints::Performance,
        experimental_features: wgpu::ExperimentalFeatures::default(),
        trace: wgpu::Trace::Off,
    }))
    // `None` means "no adapter here", which callers skip. An adapter that
    // exists and will not open a device is a failure to see, never a skip.
    .unwrap_or_else(|e| panic!("turbomap-golden: {adapter_name} refused a device: {e}"));

    Some(Gpu {
        device: Arc::new(device),
        queue: Arc::new(queue),
        adapter_name,
    })
}

/// Render once into an offscreen target and read it back as an RGBA
/// image. The `render` closure receives the encoder + target view and
/// should record one frame; the harness handles the texture readback. It
/// is renderer-agnostic on purpose — `Map`, the `TurbomapEngine`, or any
/// future engine can drive it — which is what lets the dev tooling
/// inspect (and shadow-compare) different renderers through one path.
pub fn render_to_image(
    gpu: &Gpu,
    width: u32,
    height: u32,
    render: impl FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView),
) -> RgbaImage {
    render_to_image_as(gpu, TARGET_FORMAT, width, height, render)
}

/// The target formats a renderer frame can be read back from, as an sRGB
/// 8-bit image comparable with the goldens: `Rgba8UnormSrgb` as stored, and
/// `Rgba16Float` (linear, an HDR-capable plane's format) encoded to sRGB on
/// the CPU — so the same golden holds the renderer to the same look in both.
pub fn render_to_image_as(
    gpu: &Gpu,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    mut render: impl FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView),
) -> RgbaImage {
    let bytes_per_pixel: u32 = match format {
        wgpu::TextureFormat::Rgba8UnormSrgb => 4,
        wgpu::TextureFormat::Rgba16Float => 8,
        other => {
            panic!("the golden harness reads back Rgba8UnormSrgb or Rgba16Float, not {other:?}")
        }
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("turbomap-golden-target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());

    let unpadded_bpr = width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bpr = unpadded_bpr.div_ceil(align) * align;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("turbomap-golden-readback"),
        size: (padded_bpr * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Render + copy in one encoder so the readback is this exact frame.
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    render(&mut encoder, &target_view);
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bpr),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    // No vsync in headless mode — poll the device until the map fires.
    let started = Instant::now();
    loop {
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        if let Ok(Ok(())) = rx.recv_timeout(Duration::from_millis(10)) {
            break;
        }
        assert!(
            started.elapsed() <= Duration::from_secs(10),
            "golden readback map timed out"
        );
    }
    let data = slice.get_mapped_range().unwrap_or_else(|e| {
        panic!("golden readback: the mapped range is unavailable after a successful map: {e}")
    });

    // Strip row padding back out.
    let mut tight: Vec<u8> = Vec::with_capacity((unpadded_bpr * height) as usize);
    for row in 0..height {
        let start = (row * padded_bpr) as usize;
        let end = start + unpadded_bpr as usize;
        tight.extend_from_slice(&data[start..end]);
    }
    let rgba8 = match format {
        wgpu::TextureFormat::Rgba16Float => tight
            .chunks_exact(2)
            .enumerate()
            .map(|(i, h)| {
                let v = f16_to_f32(u16::from_le_bytes([h[0], h[1]]));
                // Colour channels are linear; alpha is not encoded.
                let c = if i % 4 == 3 { v } else { linear_to_srgb(v) };
                (c.clamp(0.0, 1.0) * 255.0).round() as u8
            })
            .collect(),
        _ => tight,
    };
    RgbaImage::from_raw(width, height, rgba8).expect("golden rgba dimensions")
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// IEEE 754 binary16 → f32 (the half-float target's texels).
fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = (h >> 10) & 0x1f;
    let frac = f32::from(h & 0x3ff);
    match exp {
        0 => sign * frac * 2f32.powi(-24),
        0x1f => {
            if frac == 0.0 {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        e => sign * (1.0 + frac / 1024.0) * 2f32.powi(i32::from(e) - 15),
    }
}
