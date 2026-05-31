//! `PointerState` + `Gesture` — input-event interpretation.
//!
//! Before this lived in its own type, four separate bits of
//! state (`pointer_pos`, `drag_anchor`, `press_pos`,
//! `press_drift`) were sprinkled across `RunningState` and
//! every mouse handler manipulated some subset of them by
//! hand. The "is this a click or a pan?" rule (4-px movement
//! tolerance) appeared inline in the mouse-release branch
//! with no name.
//!
//! Here the state is encapsulated and the event handler's
//! sole job is to translate raw winit events into either a
//! `Gesture` to dispatch to the map, or nothing.

/// What the pointer-state machine determined about a window
/// event after applying the click-vs-pan rule. Yielded by the
/// `MouseInput`/`CursorMoved`/`MouseWheel` handlers.
#[derive(Debug, Clone, Copy)]
pub enum Gesture {
    /// User dragged the pointer by this many physical pixels
    /// since the previous frame. Forward to `Map::pan_by_pixels`.
    Pan { dx: f64, dy: f64 },
    /// Pointer release at `pos` that the click-tolerance rule
    /// counted as a click (not a drag). Forward to the
    /// hit-test/marker logic.
    Click { pos: (f64, f64) },
}

/// Beyond this many pixels of pointer travel between press
/// and release we treat the gesture as a pan, not a click.
const CLICK_TOLERANCE_PX: f64 = 4.0;

#[derive(Debug, Default)]
pub struct PointerState {
    /// Most recent cursor position from `WindowEvent::CursorMoved`.
    /// Used as the fallback "focus" point for scroll-zoom
    /// when the cursor isn't actually over anything yet.
    last_pos: Option<(f64, f64)>,
    /// Set on `MouseButton::Left` press, cleared on release.
    /// Each `CursorMoved` while set generates a `Pan` gesture
    /// and advances the anchor.
    drag_anchor: Option<(f64, f64)>,
    /// Position when the left button last went down. A release
    /// near this point (within `CLICK_TOLERANCE_PX`) counts
    /// as a click.
    press_pos: Option<(f64, f64)>,
    /// Accumulated absolute pointer movement since the last
    /// `Pressed`. If this exceeds `CLICK_TOLERANCE_PX` at
    /// release time, the gesture is a pan, not a click.
    press_drift: f64,
}

impl PointerState {
    /// Last-known cursor position. Used as the focus point
    /// for scroll-zoom when the wheel event arrives before
    /// the first cursor move.
    pub fn last_pos(&self) -> Option<(f64, f64)> {
        self.last_pos
    }

    /// Record a cursor move. Returns `Gesture::Pan` if we're
    /// currently in a drag.
    pub fn on_cursor_moved(&mut self, pos: (f64, f64)) -> Option<Gesture> {
        self.last_pos = Some(pos);
        let anchor = self.drag_anchor?;
        let dx = pos.0 - anchor.0;
        let dy = pos.1 - anchor.1;
        self.press_drift += (dx * dx + dy * dy).sqrt();
        self.drag_anchor = Some(pos);
        Some(Gesture::Pan { dx, dy })
    }

    /// Begin a left-button drag.
    pub fn on_left_press(&mut self) {
        self.drag_anchor = self.last_pos;
        self.press_pos = self.last_pos;
        self.press_drift = 0.0;
    }

    /// End a left-button drag. Returns `Gesture::Click` if
    /// the press-to-release pointer travel stayed within
    /// `CLICK_TOLERANCE_PX`; otherwise this was a pan and we
    /// return `None`.
    pub fn on_left_release(&mut self) -> Option<Gesture> {
        self.drag_anchor = None;
        let click = if self.press_drift <= CLICK_TOLERANCE_PX {
            self.press_pos.map(|pos| Gesture::Click { pos })
        } else {
            None
        };
        self.press_pos = None;
        self.press_drift = 0.0;
        click
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: a click and a small pan can look
    //! almost identical until the user releases. These tests
    //! pin the threshold so a future tweak to
    //! `CLICK_TOLERANCE_PX` doesn't silently change
    //! click-vs-pan semantics.
    use super::*;

    #[test]
    fn release_with_no_drift_is_a_click() {
        let mut p = PointerState::default();
        p.on_cursor_moved((100.0, 100.0));
        p.on_left_press();
        let g = p.on_left_release();
        assert!(matches!(g, Some(Gesture::Click { .. })));
    }

    #[test]
    fn release_after_tiny_drift_is_still_a_click() {
        let mut p = PointerState::default();
        p.on_cursor_moved((100.0, 100.0));
        p.on_left_press();
        // 3 px travel: under the 4 px tolerance.
        let _ = p.on_cursor_moved((103.0, 100.0));
        let g = p.on_left_release();
        assert!(matches!(g, Some(Gesture::Click { .. })));
    }

    #[test]
    fn release_after_meaningful_drag_is_not_a_click() {
        let mut p = PointerState::default();
        p.on_cursor_moved((100.0, 100.0));
        p.on_left_press();
        let _ = p.on_cursor_moved((120.0, 100.0));
        let g = p.on_left_release();
        assert!(g.is_none());
    }

    #[test]
    fn pan_is_emitted_only_while_dragging() {
        let mut p = PointerState::default();
        p.on_cursor_moved((100.0, 100.0));
        // No press yet → no pan.
        assert!(p.on_cursor_moved((110.0, 100.0)).is_none());
        p.on_left_press();
        // Inside a drag → pan emitted, delta from anchor.
        let g = p.on_cursor_moved((130.0, 110.0));
        assert!(matches!(
            g,
            Some(Gesture::Pan { dx, dy }) if (dx - 20.0).abs() < 1e-6 && (dy - 10.0).abs() < 1e-6
        ));
    }
}
