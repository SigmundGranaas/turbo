//! `RenderScheduler` — the only thing that knows *when* to
//! render. Pure state machine. No wgpu, no winit, no clocks
//! other than the `Instant` callers pass in (so it's
//! deterministically testable).
//!
//! ## Why this lives in its own type
//!
//! Before this existed, "should we render this frame?" was
//! decided by ~half a dozen booleans scattered across
//! `App::on_redraw` and `App::about_to_wait`, with implicit
//! invariants the event handlers had to maintain by hand. Every
//! attempt to fix one symptom (resize freeze, panel flicker,
//! gray startup) needed to reason about all six in parallel and
//! kept producing regressions.
//!
//! Here the rules are:
//!
//! 1. Resize events arrive in bursts (macOS fires Resized at
//!    every drag-pixel). We do NOT render during the burst —
//!    each `surface.configure` invalidates the Metal drawable
//!    pool, and the next `get_current_texture` then blocks
//!    ~1 second for the new pool. That's the multi-second
//!    "freeze" users hit during fast drags. Instead AppKit
//!    stretches the existing drawable; we apply the new size
//!    and render exactly once when the burst has been quiet
//!    for `RESIZE_SETTLE`.
//! 2. Outside of a resize, render whenever ANY dirty bit is
//!    set: pumps with worker output to drain, tiles still
//!    in-flight, animations active, egui hovering or
//!    animating.
//! 3. Idle: sleep until an OS event wakes us. No
//!    application-level rate limit — FIFO vsync inside
//!    `get_current_texture` is the only pacing we need.

use std::time::{Duration, Instant};

/// How long to wait after the last Resized event before
/// applying the new size + rendering. Bigger = smoother no-
/// freeze guarantee during very fast drags, but a bigger
/// visible delay between releasing the drag and seeing crisp
/// content. 30 ms ≈ 2 frames at 60 Hz, imperceptible.
const RESIZE_SETTLE: Duration = Duration::from_millis(30);

/// Snapshot of "what's interesting" passed in each tick. The
/// scheduler doesn't own these channels itself; the
/// dispatcher fills the struct in just before calling
/// `schedule`.
#[derive(Debug, Clone, Copy)]
pub struct Workload {
    pub workers_have_data: bool,
    pub workers_in_flight: bool,
    pub map_animating: bool,
}

/// The scheduler's decision. The dispatcher acts on it
/// without needing to inspect the scheduler's internal state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Schedule {
    /// Render this tick (caller does `request_redraw` then
    /// `on_redraw`).
    Render,
    /// Sleep until an OS event arrives.
    Idle,
    /// Sleep until this `Instant` then re-poll. Used to wake
    /// up at the end of a resize-settle window.
    WakeAt(Instant),
}

/// Pure state machine. Constructible without any GPU.
pub struct RenderScheduler {
    /// Latest size reported by `notice_resize`, with the
    /// timestamp of the most recent Resized event.
    pending_resize: Option<((u32, u32), Instant)>,
    /// Set by egui's input handler when it wants a repaint
    /// (animation, hover, etc.). Cleared each time we hand
    /// out `Render`.
    egui_dirty: bool,
}

impl RenderScheduler {
    pub fn new() -> Self {
        Self {
            pending_resize: None,
            egui_dirty: false,
        }
    }

    pub fn notice_resize(&mut self, width: u32, height: u32) {
        self.pending_resize = Some(((width, height), Instant::now()));
    }

    pub fn notice_egui_repaint(&mut self) {
        self.egui_dirty = true;
    }

    /// If a resize has been quiet for at least `RESIZE_SETTLE`,
    /// take the pending size — the caller should then
    /// reconfigure the surface and render. Returns `None`
    /// during the burst.
    pub fn take_settled_resize(&mut self, now: Instant) -> Option<(u32, u32)> {
        match self.pending_resize {
            Some((size, last_event)) if now.duration_since(last_event) >= RESIZE_SETTLE => {
                self.pending_resize = None;
                Some(size)
            }
            _ => None,
        }
    }

    /// True while we're inside the resize-burst quiet window.
    /// Renderers SHOULD NOT acquire a drawable during this
    /// time — see the module-level docs.
    pub fn in_resize_burst(&self, now: Instant) -> bool {
        match self.pending_resize {
            Some((_, last_event)) => now.duration_since(last_event) < RESIZE_SETTLE,
            None => false,
        }
    }

    /// Compute the next action given the latest workload
    /// snapshot.
    pub fn schedule(&mut self, now: Instant, workload: Workload) -> Schedule {
        if self.in_resize_burst(now) {
            // Don't render. Wake up at the end of the
            // settle window so we can apply the resize.
            let wake = self
                .pending_resize
                .map(|(_, t)| t + RESIZE_SETTLE)
                .expect("in_resize_burst implies pending_resize is set");
            return Schedule::WakeAt(wake);
        }
        let has_work = workload.workers_have_data
            || workload.workers_in_flight
            || workload.map_animating
            || self.egui_dirty
            || self.pending_resize.is_some();
        if has_work {
            self.egui_dirty = false;
            Schedule::Render
        } else {
            Schedule::Idle
        }
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: every event handler in `app.rs`
    //! mutates the scheduler, and the about-to-wait path
    //! reads back a `Schedule`. The tests below pin down the
    //! invariants that the rest of the app relies on, so
    //! future changes can't silently re-introduce the
    //! freeze/flicker/recursion bugs.
    use super::*;

    fn workload_idle() -> Workload {
        Workload {
            workers_have_data: false,
            workers_in_flight: false,
            map_animating: false,
        }
    }

    #[test]
    fn idle_with_nothing_to_do_is_idle() {
        let mut s = RenderScheduler::new();
        assert_eq!(s.schedule(Instant::now(), workload_idle()), Schedule::Idle);
    }

    #[test]
    fn worker_data_triggers_render() {
        let mut s = RenderScheduler::new();
        let mut w = workload_idle();
        w.workers_have_data = true;
        assert_eq!(s.schedule(Instant::now(), w), Schedule::Render);
    }

    #[test]
    fn egui_repaint_triggers_render_once() {
        let mut s = RenderScheduler::new();
        s.notice_egui_repaint();
        assert_eq!(
            s.schedule(Instant::now(), workload_idle()),
            Schedule::Render
        );
        // Dirty bit consumed by the render.
        assert_eq!(s.schedule(Instant::now(), workload_idle()), Schedule::Idle);
    }

    #[test]
    fn resize_burst_returns_wake_not_render() {
        let mut s = RenderScheduler::new();
        let t0 = Instant::now();
        s.notice_resize(800, 600);
        match s.schedule(t0, workload_idle()) {
            Schedule::WakeAt(wake) => {
                assert!(wake > t0);
            }
            other => panic!("expected WakeAt, got {other:?}"),
        }
        // Even with worker data, we still defer to after the
        // settle window — rendering during the burst would
        // exhaust the Metal drawable pool.
        let mut w = workload_idle();
        w.workers_have_data = true;
        assert!(matches!(s.schedule(t0, w), Schedule::WakeAt(_)));
    }

    #[test]
    fn resize_settle_then_render() {
        let mut s = RenderScheduler::new();
        let t0 = Instant::now();
        s.notice_resize(800, 600);
        let after = t0 + RESIZE_SETTLE + Duration::from_millis(1);
        // Settle elapsed: scheduler should hand the resize to
        // the caller and report Render.
        assert_eq!(s.take_settled_resize(after), Some((800, 600)));
        // After the resize was taken, the scheduler is idle.
        assert_eq!(s.schedule(after, workload_idle()), Schedule::Idle);
    }

    #[test]
    fn rapid_resize_keeps_extending_quiet_window() {
        let mut s = RenderScheduler::new();
        let t0 = Instant::now();
        s.notice_resize(800, 600);
        // Almost-but-not-quite settled.
        let almost = t0 + RESIZE_SETTLE - Duration::from_millis(1);
        assert!(s.in_resize_burst(almost));
        // Another event arrives: the quiet window resets.
        s.notice_resize(900, 700);
        // The previous "almost settled" instant is no longer
        // past the new last-event timestamp.
        assert!(s.take_settled_resize(almost).is_none());
        assert!(s.in_resize_burst(almost));
    }
}
