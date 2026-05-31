//! `RenderSurface` — the only thing that knows how to acquire a
//! wgpu drawable and keep `wgpu::SurfaceConfiguration` in sync
//! with the window's actual size.
//!
//! ## Why this lives in its own type
//!
//! Before this existed, every `WindowEvent::Resized` did a
//! `surface.configure()` inline, and `WindowEvent::RedrawRequested`
//! called `get_current_texture` separately. Two code paths
//! contended for a single CAMetalLayer + drawable pool. When
//! macOS fired Resized events faster than the drawable pool
//! could be reallocated (~1 s per `configure()` worth of GPU
//! work), `get_current_texture` started returning `Outdated`
//! repeatedly and the main thread spun for the entire duration
//! of the user's drag. By moving both responsibilities here we
//! get one rule for the surface's lifecycle:
//!
//! 1. Configure is **lazy**. We never call `configure` from an
//!    event handler. Instead, callers tell us the latest
//!    target size; the next `acquire` does the configure if
//!    needed.
//! 2. `Outdated`/`Lost` recovery re-reads the window's actual
//!    physical size from the `Window` itself (the authoritative
//!    source) before reconfiguring. The previous code
//!    reconfigured to a stale `surface_config` and looped.
//! 3. Acquire returns `None` on transient failure — never
//!    panics, never blocks the caller waiting. The caller's
//!    next tick gets the frame.

use std::sync::Arc;

use wgpu::TextureFormat;

/// Owner of the wgpu surface + its configuration. Hides
/// reconfigure timing and Lost/Outdated handling from callers.
pub struct RenderSurface {
    device: Arc<wgpu::Device>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// Cached so we can clamp incoming sizes without going
    /// back to the device every time.
    max_dim: u32,
}

/// Result of [`RenderSurface::acquire`]. Holds the wgpu
/// surface texture + a default view; calling code can use the
/// view directly in a render pass and ignore the underlying
/// texture lifetime.
pub struct SurfaceFrame {
    pub texture: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
    pub size: (u32, u32),
}

impl SurfaceFrame {
    pub fn present(self) {
        self.texture.present();
    }
}

impl RenderSurface {
    pub fn new(
        device: Arc<wgpu::Device>,
        surface: wgpu::Surface<'static>,
        format: TextureFormat,
        alpha_mode: wgpu::CompositeAlphaMode,
        initial_size: (u32, u32),
        max_dim: u32,
    ) -> Self {
        let (w, h) = clamp_size(initial_size, max_dim);
        let config = wgpu::SurfaceConfiguration {
            // RENDER_ATTACHMENT for normal rendering, COPY_SRC
            // so the diagnostic framebuffer dump (see
            // `app::dump_frame_to_png`) can read back what the
            // GPU produced for each frame — the only way to
            // distinguish "GPU rendered different pixels" from
            // "compositor presented stale pixels" when chasing
            // a flicker.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: w,
            height: h,
            // Strict FIFO vsync. `AutoVsync` on Metal can pick
            // Mailbox and produce beat-frequency flicker
            // against the display's actual refresh rate.
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        Self {
            device,
            surface,
            config,
            max_dim,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn format(&self) -> TextureFormat {
        self.config.format
    }

    /// Reconfigure to `(width, height)` if it differs from
    /// the current configuration. Cheap no-op if the size is
    /// already current. Callers should invoke this just
    /// before `acquire` when they know a resize has settled —
    /// see `RenderScheduler::take_settled_resize`.
    pub fn resize_to(&mut self, width: u32, height: u32) {
        let (w, h) = clamp_size((width, height), self.max_dim);
        if w == self.config.width && h == self.config.height {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
    }

    /// Try to acquire a drawable. On `Outdated`/`Lost`, re-
    /// sync to `window_size_hint` (the window's actual
    /// physical size from `Window::inner_size`) and return
    /// `None`. Callers should retry on the next tick.
    pub fn acquire(&mut self, window_size_hint: (u32, u32)) -> Option<SurfaceFrame> {
        match self.surface.get_current_texture() {
            Ok(tex) => {
                let view = tex
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let size = (self.config.width, self.config.height);
                Some(SurfaceFrame {
                    texture: tex,
                    view,
                    size,
                })
            }
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // The CAMetalLayer dimension diverged from
                // our configured surface — most likely because
                // AppKit just resized the window. Sync to the
                // window's authoritative size, then return
                // `None` so the caller retries with a fresh
                // drawable next tick. We DELIBERATELY do not
                // recurse / re-acquire here: the freshly-
                // reconfigured pool needs a moment to allocate
                // and trying to read it immediately can block
                // the main thread for ~1 second per call.
                self.resize_to(window_size_hint.0, window_size_hint.1);
                None
            }
            Err(e) => {
                log::warn!("surface acquire error: {e:?}");
                None
            }
        }
    }
}

fn clamp_size((w, h): (u32, u32), max_dim: u32) -> (u32, u32) {
    (w.min(max_dim).max(1), h.min(max_dim).max(1))
}
