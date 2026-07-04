//! `UiOverlay` — owns the entire egui integration.
//!
//! Before this existed, five separate egui call sites lived
//! inline in `App::on_redraw`:
//!
//! 1. `egui_state.take_egui_input`
//! 2. `egui_ctx.run` with the panel-building closure
//! 3. `egui_state.handle_platform_output`
//! 4. `egui_renderer.update_buffers`
//! 5. `egui_renderer.render` inside a render pass
//! 6. `egui_renderer.free_texture` for retired textures
//!
//! Any change to the egui integration touched all six. They
//! also shared a constraint with the rest of the frame —
//! `free_texture` must happen AFTER `queue.submit` or we free
//! atlas glyphs the GPU is still sampling — which used to be
//! enforced by a comment instead of by a type. Now the type
//! enforces it: `frame` builds the egui output and returns a
//! `PendingUi` that holds the textures-to-free; the caller
//! invokes `present(pending)` AFTER its submit and the borrow
//! checker won't let it forget.

use std::sync::Arc;

use winit::{event::WindowEvent, window::Window};

pub struct UiOverlay {
    ctx: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
}

/// Output of a frame's egui run. Held by the caller until
/// after the render submission so retired textures aren't
/// freed while the GPU still references them.
pub struct PendingUi {
    free_textures: Vec<egui::TextureId>,
    wants_repaint: bool,
}

impl PendingUi {
    /// True if egui requested another frame "now"
    /// (animation, hover effect, freshly-typed char).
    pub fn wants_repaint(&self) -> bool {
        self.wants_repaint
    }
}

/// Result of `on_window_event` — what the app needs to know
/// about the event after egui has consumed it.
#[derive(Debug, Clone, Copy)]
pub struct OverlayEventResponse {
    /// egui has captured this event (e.g. clicking on a
    /// slider). The app's own input handling should treat it
    /// as consumed.
    pub egui_used_pointer: bool,
    /// egui wants a repaint (animation, tooltip, etc.). The
    /// scheduler should be notified.
    pub repaint_requested: bool,
}

impl UiOverlay {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        window: &Window,
    ) -> Self {
        let ctx = egui::Context::default();
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let renderer = egui_wgpu::Renderer::new(
            device,
            surface_format,
            egui_wgpu::RendererOptions::default(),
        );
        Self {
            ctx,
            state,
            renderer,
        }
    }

    /// Feed a window event into egui. Caller should use the
    /// returned response to gate its own input handling (so
    /// dragging a slider doesn't also pan the map) and to
    /// signal the scheduler if a repaint was requested.
    pub fn on_window_event(
        &mut self,
        window: &Window,
        event: &WindowEvent,
    ) -> OverlayEventResponse {
        let resp = self.state.on_window_event(window, event);
        OverlayEventResponse {
            egui_used_pointer: self.ctx.wants_pointer_input(),
            repaint_requested: resp.repaint,
        }
    }

    /// True while a focused egui widget is consuming keyboard
    /// input (e.g. a text edit). Used by the app's keyboard
    /// shortcut handler to avoid firing global shortcuts
    /// while the user is typing into the panel.
    pub fn wants_keyboard(&self) -> bool {
        self.ctx.wants_keyboard_input()
    }

    /// Build one egui frame using `build_ui` and encode the
    /// draw calls into `encoder`. The returned `PendingUi`
    /// must be passed to `present` AFTER `queue.submit`.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        device: &Arc<wgpu::Device>,
        queue: &Arc<wgpu::Queue>,
        window: &Window,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        surface_size: (u32, u32),
        build_ui: impl FnOnce(&egui::Context),
    ) -> PendingUi {
        let input = self.state.take_egui_input(window);
        // `Context::run` takes an `FnMut`, so we hold the
        // build closure in a slot and have the inner FnMut
        // pull it out exactly once.
        let mut build_once = Some(build_ui);
        let output = self.ctx.run(input, |ctx| {
            if let Some(build) = build_once.take() {
                build(ctx);
            }
        });
        self.state
            .handle_platform_output(window, output.platform_output);

        let pixels_per_point = self.ctx.pixels_per_point();
        let clipped = self.ctx.tessellate(output.shapes, pixels_per_point);
        let screen_desc = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [surface_size.0, surface_size.1],
            pixels_per_point,
        };
        for (id, image_delta) in &output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }
        self.renderer
            .update_buffers(device, queue, encoder, &clipped, &screen_desc);
        // Scope the render pass tightly. `forget_lifetime`
        // is required by egui-wgpu's 'static signature.
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("turbomap-egui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let mut pass = pass.forget_lifetime();
        self.renderer.render(&mut pass, &clipped, &screen_desc);
        drop(pass);

        let wants_repaint = output
            .viewport_output
            .values()
            .any(|v| v.repaint_delay.is_zero());

        PendingUi {
            free_textures: output.textures_delta.free,
            wants_repaint,
        }
    }

    /// Run AFTER `queue.submit` for the frame. Frees egui
    /// textures that were retired this frame. Doing it
    /// before submit would free a font atlas the GPU is
    /// still sampling — that produced a per-frame black
    /// flicker on the panel.
    pub fn present(&mut self, pending: PendingUi) -> bool {
        for id in &pending.free_textures {
            self.renderer.free_texture(id);
        }
        pending.wants_repaint
    }
}
