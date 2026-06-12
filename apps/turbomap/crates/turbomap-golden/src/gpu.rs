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

/// Build a headless context, preferring a software adapter for
/// determinism. Returns `None` if no adapter can be acquired.
pub fn headless() -> Option<Gpu> {
    let instance = wgpu::Instance::new({
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        desc.backends = wgpu::Backends::PRIMARY | wgpu::Backends::GL;
        desc
    });

    // Prefer the fallback (software) adapter — deterministic across
    // machines. Fall back to whatever exists so a dev box with only a
    // hardware GPU can still run the harness (with looser tolerances).
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: true,
    }))
    .ok()
    .or_else(|| {
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok()
    })?;

    let adapter_name = adapter.get_info().name;

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("turbomap-golden-device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        memory_hints: wgpu::MemoryHints::Performance,
        experimental_features: wgpu::ExperimentalFeatures::default(),
        trace: wgpu::Trace::Off,
    }))
    .ok()?;

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
    mut render: impl FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView),
) -> RgbaImage {
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
        format: TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());

    let bytes_per_pixel = 4u32;
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
    let data = slice.get_mapped_range();

    // Strip row padding back out.
    let mut tight: Vec<u8> = Vec::with_capacity((unpadded_bpr * height) as usize);
    for row in 0..height {
        let start = (row * padded_bpr) as usize;
        let end = start + unpadded_bpr as usize;
        tight.extend_from_slice(&data[start..end]);
    }
    RgbaImage::from_raw(width, height, tight).expect("golden rgba dimensions")
}
