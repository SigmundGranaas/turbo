//! The frame graph — the declarative skeleton of a rendered frame
//! (architecture §III.3, slice D1).
//!
//! Every piece of per-frame GPU work is a **pass**: a name, a phase, and the
//! logical resources it reads and writes ([`PassDesc`]). `Map::render` no
//! longer hand-sequences its work; it registers passes with a [`FrameGraph`]
//! and the graph runs them. What that buys over the hand-written sequence:
//!
//! * **One inspectable structure.** The frame is a list of named passes with
//!   declared data flow — dumpable ([`FrameGraphReport`]), assertable
//!   ([`validate`]), and diffable across builds.
//! * **Per-pass timing, always on.** Each pass's CPU encode time lands in
//!   [`PassTiming`]s on the frame metrics, so "the frame got slow" decomposes
//!   into *which pass* without a profiler attached.
//! * **Pass isolation.** Any pass can be disabled at runtime by name
//!   ([`PassMask`]) — render just the terrain, kill the sky, drop one layer —
//!   which is the debug story for evaluating subsystems one by one.
//! * **The single-MSAA-pass invariant is structural.** On tile-based mobile
//!   GPUs every render pass costs a full framebuffer load/store, so all
//!   ground + overlay draw contributions MUST share one wgpu render pass.
//!   The graph owns that pass ([`FrameGraph::run_msaa`]); contributions get a
//!   `&mut wgpu::RenderPass` and *cannot* begin their own. Composite passes
//!   (clouds) that legitimately need another pass are a different phase, and
//!   the extra cost is visible per-pass in the report.
//!
//! Scheduling: phases run in declaration order ([`FramePhase`] is ordered);
//! within the shared MSAA pass, GroundMsaa contributions run before
//! OverlayMsaa. Within a phase, registration order is preserved — a map is a
//! painter's algorithm, and painter's order (sky → ground → overlays) is
//! semantic, not incidental. The reads/writes declarations are validated
//! (every read must be produced this frame or be a persistent resource), so a
//! mis-ordered registration fails loudly in tests instead of rendering wrong.

use std::collections::BTreeSet;
use std::time::Duration;
// `std::time::Instant` panics on wasm ("time not implemented"); web_time is
// std's Instant on native and `performance.now()` in the browser. Per-pass
// profiling must never be the thing that breaks a platform.
use web_time::Instant;

/// Logical resources a pass can read or write. Coarse on purpose: these tag
/// *data flow between passes*, not individual GPU objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Res {
    /// The camera-centred cross-tile relief heightfield (shadow map texture).
    /// Persistent: reassembled only when the camera settles in a new region
    /// or the DEM changes; consumers may read last frame's field.
    HeightField,
    /// The world-locked ambient-occlusion field baked from the heightfield.
    /// Persistent: refined progressively, cached across frames.
    AoField,
    /// Per-frame shadow/lighting uniforms fed into the terrain config.
    ShadowUniforms,
    /// The multisampled colour attachment of the frame pass.
    ColorMsaa,
    /// The multisampled depth attachment of the frame pass.
    Depth,
    /// The frame's resolved, single-sampled target (the surface).
    FrameTarget,
}

impl Res {
    /// Persistent resources survive across frames: a pass may read them even
    /// if nothing wrote them *this* frame.
    fn persistent(self) -> bool {
        matches!(self, Res::HeightField | Res::AoField)
    }
}

/// Where in the frame a pass runs. Ordered: earlier variants run first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FramePhase {
    /// Encoder-level work before the frame pass: heightfield assembly,
    /// AO accumulation. May use the encoder but not the frame attachments.
    BeforeFrame,
    /// Ground content inside the shared MSAA pass: sky, floor, tile layers,
    /// route tubes. Depth-tested world geometry.
    GroundMsaa,
    /// Screen-space overlays inside the shared MSAA pass: icons, labels,
    /// markers. Drawn after all ground content.
    OverlayMsaa,
    /// Encoder-level work after the frame pass resolves, compositing onto the
    /// resolved target (clouds). Each composite pass is an extra fullscreen
    /// pass — the expensive kind — which is why it's a separate, visible phase.
    Composite,
}

/// The declarative description of a pass: its identity and data flow.
/// `'static` by design — the set of pass *kinds* is a compile-time property
/// of the renderer (dynamic instances carry a `detail` label, e.g. one node
/// per tile layer).
pub struct PassDesc {
    pub name: &'static str,
    pub phase: FramePhase,
    pub reads: &'static [Res],
    pub writes: &'static [Res],
}

/// Runtime pass disable-set, keyed by pass name (or `name:detail` for a
/// specific instance, e.g. `layer:hillshade`). Disabling a pass skips its
/// execution but keeps its bookkeeping (it shows as `skipped` in the report),
/// so an isolation experiment is still a fully described frame.
#[derive(Debug, Default, Clone)]
pub struct PassMask {
    disabled: BTreeSet<String>,
}

impl PassMask {
    pub fn set_enabled(&mut self, name: &str, enabled: bool) {
        if enabled {
            self.disabled.remove(name);
        } else {
            self.disabled.insert(name.to_string());
        }
    }

    /// A pass instance runs unless its name OR its full `name:detail` label
    /// is disabled (so `layer` kills every tile layer, `layer:hillshade` just
    /// one).
    fn enabled(&self, name: &str, detail: Option<&str>) -> bool {
        if self.disabled.contains(name) {
            return false;
        }
        match detail {
            Some(d) => !self.disabled.contains(&format!("{name}:{d}")),
            None => true,
        }
    }

    pub fn disabled_passes(&self) -> impl Iterator<Item = &str> {
        self.disabled.iter().map(|s| s.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.disabled.is_empty()
    }
}

/// One executed (or skipped) pass instance in the frame report.
#[derive(Debug, Clone)]
pub struct PassTiming {
    /// `name` or `name:detail` — unique per instance within the frame.
    pub label: String,
    pub phase: FramePhase,
    /// CPU wall time spent encoding/executing the pass. For draw
    /// contributions this is encode time, not GPU time.
    pub cpu: Duration,
    /// True when the pass was masked off this frame.
    pub skipped: bool,
}

/// The attachments + clear for the shared MSAA frame pass. Owned here so the
/// "one wgpu pass for all ground + overlay work" rule has a single home.
pub(crate) struct MsaaAttachments<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub resolve_target: &'a wgpu::TextureView,
    pub depth_view: &'a wgpu::TextureView,
    pub clear: wgpu::Color,
}

/// A deferred draw contribution to the shared MSAA pass.
struct DrawNode<'f> {
    desc: &'static PassDesc,
    detail: Option<String>,
    run: Box<dyn FnOnce(&mut wgpu::RenderPass<'_>) + 'f>,
}

/// The deferred contributions to the shared MSAA pass, built up during
/// registration and consumed whole by [`FrameGraph::run_msaa`]. A separate,
/// short-lived object (rather than state inside [`FrameGraph`]) so the
/// borrows its closures hold over the `Map` end at `run_msaa` — later
/// composite passes may then take `&mut` state again.
pub(crate) struct DrawList<'f> {
    nodes: Vec<DrawNode<'f>>,
}

impl<'f> DrawList<'f> {
    pub(crate) fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(12),
        }
    }

    /// Register a draw contribution. `detail` distinguishes instances of the
    /// same pass kind (`layer:basemap`, `layer:hillshade`).
    pub(crate) fn add(
        &mut self,
        desc: &'static PassDesc,
        detail: Option<String>,
        f: impl FnOnce(&mut wgpu::RenderPass<'_>) + 'f,
    ) {
        debug_assert!(
            matches!(desc.phase, FramePhase::GroundMsaa | FramePhase::OverlayMsaa),
            "only Ground/Overlay contributions may join the MSAA pass"
        );
        self.nodes.push(DrawNode {
            desc,
            detail,
            run: Box::new(f),
        });
    }
}

/// Record of a pass that ran (for validation).
struct Executed {
    reads: &'static [Res],
    writes: &'static [Res],
    label: String,
    skipped: bool,
}

/// Per-frame graph. Built fresh in `Map::render`; immediate passes
/// ([`Self::run_now`], [`Self::run_encoder`]) execute as they're registered
/// (they need `&mut Map` state the borrow checker won't let us hold across
/// the frame), draw contributions are collected in a [`DrawList`] and run
/// inside the single MSAA pass by [`Self::run_msaa`]. Declarative
/// bookkeeping (timing, mask, validation) is identical for both.
pub(crate) struct FrameGraph {
    mask: PassMask,
    executed: Vec<Executed>,
    timings: Vec<PassTiming>,
}

/// The finished frame's pass report: what ran, in what order, how long.
#[derive(Debug, Clone, Default)]
pub struct FrameGraphReport {
    pub passes: Vec<PassTiming>,
}

impl FrameGraph {
    pub(crate) fn new(mask: PassMask) -> Self {
        Self {
            mask,
            executed: Vec::with_capacity(16),
            timings: Vec::with_capacity(16),
        }
    }

    fn label(desc: &PassDesc, detail: Option<&str>) -> String {
        match detail {
            Some(d) => format!("{}:{d}", desc.name),
            None => desc.name.to_string(),
        }
    }

    fn record(
        &mut self,
        desc: &'static PassDesc,
        detail: Option<&str>,
        cpu: Duration,
        skipped: bool,
    ) {
        let label = Self::label(desc, detail);
        self.executed.push(Executed {
            reads: desc.reads,
            writes: desc.writes,
            label: label.clone(),
            skipped,
        });
        self.timings.push(PassTiming {
            label,
            phase: desc.phase,
            cpu,
            skipped,
        });
    }

    /// Run a CPU-side pass immediately (no encoder): heightfield sampling,
    /// queue uploads. Returns `None` when the pass is masked off.
    pub(crate) fn run_now<R>(
        &mut self,
        desc: &'static PassDesc,
        f: impl FnOnce() -> R,
    ) -> Option<R> {
        debug_assert!(matches!(
            desc.phase,
            FramePhase::BeforeFrame | FramePhase::Composite
        ));
        if !self.mask.enabled(desc.name, None) {
            self.record(desc, None, Duration::ZERO, true);
            return None;
        }
        let t0 = Instant::now();
        let r = f();
        self.record(desc, None, t0.elapsed(), false);
        Some(r)
    }

    /// Run an encoder-level pass immediately (its own wgpu pass(es), outside
    /// the shared MSAA pass): AO accumulation, cloud composite.
    pub(crate) fn run_encoder(
        &mut self,
        desc: &'static PassDesc,
        encoder: &mut wgpu::CommandEncoder,
        f: impl FnOnce(&mut wgpu::CommandEncoder),
    ) {
        debug_assert!(matches!(
            desc.phase,
            FramePhase::BeforeFrame | FramePhase::Composite
        ));
        if !self.mask.enabled(desc.name, None) {
            self.record(desc, None, Duration::ZERO, true);
            return;
        }
        let t0 = Instant::now();
        f(encoder);
        self.record(desc, None, t0.elapsed(), false);
    }

    /// Begin THE frame's single MSAA render pass, run every registered ground
    /// contribution then every overlay contribution into it, and resolve to
    /// the frame target. This is the only place in the renderer that begins
    /// the frame pass — the one-pass-per-frame rule (tile-GPU load/store
    /// cost) is enforced by construction.
    ///
    /// Ordering: GroundMsaa before OverlayMsaa (phase order); within a phase,
    /// registration order (painter's order is semantic). The sort is stable.
    pub(crate) fn run_msaa(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        attachments: MsaaAttachments<'_>,
        draws: DrawList<'_>,
    ) {
        let mut nodes = draws.nodes;
        let order = msaa_order(&nodes.iter().map(|n| n.desc.phase).collect::<Vec<_>>());
        // Apply the schedule: drain in computed order (indices are a permutation).
        let mut slots: Vec<Option<DrawNode<'_>>> = nodes.drain(..).map(Some).collect();
        let nodes: Vec<DrawNode<'_>> = order
            .into_iter()
            .map(|i| slots[i].take().expect("permutation"))
            .collect();

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("turbomap-frame-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: attachments.color_view,
                resolve_target: Some(attachments.resolve_target),
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(attachments.clear),
                    store: wgpu::StoreOp::Discard,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: attachments.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        for node in nodes {
            let skipped = !self.mask.enabled(node.desc.name, node.detail.as_deref());
            if skipped {
                self.record(node.desc, node.detail.as_deref(), Duration::ZERO, true);
                continue;
            }
            let t0 = Instant::now();
            (node.run)(&mut pass);
            self.record(node.desc, node.detail.as_deref(), t0.elapsed(), false);
        }
        drop(pass);
        // The pass's clear + end-of-pass MSAA resolve produce the frame
        // target — record it so composite passes' `FrameTarget` reads
        // validate. Bookkeeping-only (no timing entry): the cost is inside
        // the contributions above.
        self.executed.push(Executed {
            reads: &[],
            writes: &[Res::FrameTarget],
            label: "frame-pass-resolve".to_string(),
            skipped: false,
        });
    }

    /// Validate the executed frame's data flow: every non-persistent read
    /// must have been written by an earlier, non-skipped pass this frame.
    /// Skipped passes don't produce; their consumers reading a persistent
    /// resource still pass (stale-but-valid is exactly the isolation-debug
    /// contract).
    pub(crate) fn validate(&self) -> Result<(), String> {
        let mut written: BTreeSet<Res> = BTreeSet::new();
        for ex in &self.executed {
            if ex.skipped {
                continue;
            }
            for r in ex.reads {
                if !r.persistent() && !written.contains(r) {
                    return Err(format!(
                        "pass '{}' reads {:?} before any pass wrote it this frame",
                        ex.label, r
                    ));
                }
            }
            for w in ex.writes {
                written.insert(*w);
            }
        }
        Ok(())
    }

    /// Consume the graph, returning the frame's pass report. Debug builds
    /// assert the data-flow validation here so a mis-ordered port fails in
    /// every test that renders a frame.
    pub(crate) fn finish(self) -> FrameGraphReport {
        debug_assert!(self.validate().is_ok(), "{:?}", self.validate());
        FrameGraphReport {
            passes: self.timings,
        }
    }
}

/// The MSAA-pass schedule: contributions sorted by phase (GroundMsaa before
/// OverlayMsaa), stable within a phase (painter's order is registration
/// order). Pure so the scheduler is unit-testable without a GPU.
fn msaa_order(phases: &[FramePhase]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..phases.len()).collect();
    idx.sort_by_key(|&i| phases[i]);
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEFORE: PassDesc = PassDesc {
        name: "shadow-assemble",
        phase: FramePhase::BeforeFrame,
        reads: &[],
        writes: &[Res::HeightField, Res::ShadowUniforms],
    };
    const GROUND: PassDesc = PassDesc {
        name: "layer",
        phase: FramePhase::GroundMsaa,
        reads: &[Res::ShadowUniforms],
        writes: &[Res::ColorMsaa, Res::Depth],
    };

    #[test]
    fn mask_disables_by_name_and_by_instance() {
        let mut m = PassMask::default();
        m.set_enabled("layer:hillshade", false);
        assert!(m.enabled("layer", Some("basemap")));
        assert!(!m.enabled("layer", Some("hillshade")));
        m.set_enabled("layer", false);
        assert!(!m.enabled("layer", Some("basemap")));
        m.set_enabled("layer", true);
        assert!(m.enabled("layer", Some("basemap")));
    }

    #[test]
    fn run_now_times_and_masks() {
        let mut g = FrameGraph::new(PassMask::default());
        let ran = g.run_now(&BEFORE, || 42);
        assert_eq!(ran, Some(42));
        let report = g.finish();
        assert_eq!(report.passes.len(), 1);
        assert_eq!(report.passes[0].label, "shadow-assemble");
        assert!(!report.passes[0].skipped);

        let mut mask = PassMask::default();
        mask.set_enabled("shadow-assemble", false);
        let mut g = FrameGraph::new(mask);
        let ran = g.run_now(&BEFORE, || 42);
        assert_eq!(ran, None);
        let report = g.finish();
        assert!(report.passes[0].skipped);
    }

    #[test]
    fn validate_rejects_read_before_write() {
        // A ground pass reading ShadowUniforms with the producer masked off
        // must fail validation… unless the resource is persistent.
        let mut mask = PassMask::default();
        mask.set_enabled("shadow-assemble", false);
        let mut g = FrameGraph::new(mask);
        assert!(g.run_now(&BEFORE, || ()).is_none());
        // Simulate the ground pass having executed (validation is over the
        // executed record; we don't need a real wgpu pass for this).
        g.executed.push(Executed {
            reads: GROUND.reads,
            writes: GROUND.writes,
            label: "layer:basemap".into(),
            skipped: false,
        });
        assert!(g.validate().is_err());

        // Same shape but reading a persistent resource: fine.
        g.executed.last_mut().unwrap().reads = &[Res::HeightField];
        assert!(g.validate().is_ok());
    }

    #[test]
    fn scheduler_orders_overlay_after_ground_stably() {
        use FramePhase::{GroundMsaa as G, OverlayMsaa as O};
        // Registration interleaves overlay + ground; the schedule must put
        // every ground contribution first while preserving relative order
        // within each phase.
        let phases = [O, G, O, G, G, O];
        assert_eq!(msaa_order(&phases), vec![1, 3, 4, 0, 2, 5]);
        // Already-ordered input is untouched.
        let phases = [G, G, O, O];
        assert_eq!(msaa_order(&phases), vec![0, 1, 2, 3]);
    }

    #[test]
    fn validate_accepts_writer_before_reader() {
        let mut g = FrameGraph::new(PassMask::default());
        assert!(g.run_now(&BEFORE, || ()).is_some());
        g.executed.push(Executed {
            reads: GROUND.reads,
            writes: GROUND.writes,
            label: "layer:basemap".into(),
            skipped: false,
        });
        assert!(g.validate().is_ok());
    }
}
