//! `GpuContext` — the root of every GPU resource the app owns.
//!
//! `wgpu` follows a deliberate hierarchy: `Instance` → `Adapter`
//! → `Device` + `Queue`. Surfaces, textures, buffers, bind groups
//! all hang off the device; everything that uses the GPU needs
//! at least a clone of the `Arc<Device>` / `Arc<Queue>`.
//!
//! Before this type existed, those handles floated around as
//! ad-hoc `Arc` clones passed into half a dozen constructors
//! during `App::resumed`. There was no answer to "where do
//! `device` and `queue` actually live?", which made it easy to
//! drift into ambiguous ownership patterns. Now there's one
//! answer: `GpuContext`, owned by the `App`, cloned where
//! needed.
//!
//! `GpuContext` is intentionally NOT generic over backend. The
//! choice to target Metal/Vulkan/DX12 is made by
//! `Backends::PRIMARY` here and nowhere else.

use std::sync::Arc;

use winit::window::Window;

/// Top-level handle to every wgpu resource. Construct once in
/// `App::resumed` and clone the `Arc`s into anything that
/// needs them (the render surface, the map, the egui renderer,
/// …).
pub struct GpuContext {
    /// Held so the adapter + surfaces it created stay valid.
    /// Otherwise unused after construction.
    #[allow(dead_code)]
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    /// Surface texture format the swapchain was configured
    /// with — pipelines need this at construction so callers
    /// don't have to read it back from `RenderSurface`.
    pub surface_format: wgpu::TextureFormat,
    /// Composite alpha mode supported by this adapter for the
    /// constructed surface. Passed through to the surface
    /// configuration.
    pub alpha_mode: wgpu::CompositeAlphaMode,
    /// The surface itself — handed to `RenderSurface` at
    /// construction; not held here after that. Wrapped in an
    /// `Option` so `into_surface` can move it out.
    surface: Option<wgpu::Surface<'static>>,
}

impl GpuContext {
    /// Build a GPU context whose surface is attached to
    /// `window`. The window's size is used as the initial
    /// surface configuration target; the actual configuration
    /// happens later in `RenderSurface::new`.
    pub fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new({
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        desc.backends = wgpu::Backends::PRIMARY;
        desc
    });
        let surface = instance
            .create_surface(window)
            .expect("create surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("request adapter");
        // Opt into TIMESTAMP_QUERY when the adapter offers it
        // so the Map can emit GPU wall time in its
        // FrameMetrics. Adapters without it (some Vulkan/D3D
        // drivers, WebGPU on some browsers) simply skip the
        // readback.
        let wanted =
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        let features = adapter.features() & wanted;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("turbomap-device"),
            required_features: features,
            required_limits: wgpu::Limits::downlevel_defaults()
                .using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: wgpu::ExperimentalFeatures::default(),
            trace: wgpu::Trace::Off,
        }))
        .expect("request device");
        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let alpha_mode = caps.alpha_modes[0];
        Self {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
            surface_format,
            alpha_mode,
            surface: Some(surface),
        }
    }

    /// Maximum 2D texture dimension supported by the device.
    /// Used to clamp resize requests before configuring the
    /// surface.
    pub fn max_texture_dimension_2d(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }

    /// Move the surface out of the context. Called once by
    /// `App::resumed` when constructing the `RenderSurface`.
    /// Panics if called twice — there is only one surface and
    /// only one owner downstream.
    pub fn take_surface(&mut self) -> wgpu::Surface<'static> {
        self.surface
            .take()
            .expect("GpuContext::take_surface called twice")
    }
}
