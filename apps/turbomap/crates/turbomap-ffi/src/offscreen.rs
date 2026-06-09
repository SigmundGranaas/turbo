//! Headless GPU context + offscreen readback for the FFI layer.
//!
//! The FFI control plane never receives a native drawable — surface
//! creation is per-platform glue outside uniffi. What it *can* do
//! everywhere is render offscreen: that powers the `render_png` snapshot
//! API (FFI-level verification from any host language) and headless
//! embedding (tests, server-side rendering). Prefers a software adapter
//! for determinism, falls back to whatever the machine has.

use std::sync::Arc;
use std::time::{Duration, Instant};

/// The colour-correct format the rest of the renderer targets.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub struct GpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub adapter_name: String,
}

/// Acquire a headless context, or `None` if no adapter exists.
pub fn headless() -> Option<GpuContext> {
    let instance = wgpu::Instance::new({
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        desc.backends = wgpu::Backends::PRIMARY | wgpu::Backends::GL;
        desc
    });
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
        label: Some("turbomap-ffi-device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        memory_hints: wgpu::MemoryHints::Performance,
        experimental_features: wgpu::ExperimentalFeatures::default(),
        trace: wgpu::Trace::Off,
    }))
    .ok()?;
    Some(GpuContext {
        device: Arc::new(device),
        queue: Arc::new(queue),
        adapter_name,
    })
}

/// Render one frame via `record` into an offscreen target and read the
/// pixels back as tightly-packed RGBA8.
pub fn render_to_rgba(
    gpu: &GpuContext,
    width: u32,
    height: u32,
    mut record: impl FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView),
) -> Result<Vec<u8>, String> {
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("turbomap-ffi-target"),
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
    let view = target.create_view(&Default::default());

    let bytes_per_pixel = 4u32;
    let unpadded_bpr = width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bpr = unpadded_bpr.div_ceil(align) * align;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("turbomap-ffi-readback"),
        size: (padded_bpr * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    record(&mut encoder, &view);
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
    let started = Instant::now();
    loop {
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        if let Ok(Ok(())) = rx.recv_timeout(Duration::from_millis(10)) {
            break;
        }
        if started.elapsed() > Duration::from_secs(10) {
            return Err("readback map timed out".to_string());
        }
    }
    let data = slice.get_mapped_range();
    let mut tight = Vec::with_capacity((unpadded_bpr * height) as usize);
    for row in 0..height {
        let start = (row * padded_bpr) as usize;
        tight.extend_from_slice(&data[start..start + unpadded_bpr as usize]);
    }
    Ok(tight)
}
