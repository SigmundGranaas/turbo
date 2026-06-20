package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import androidx.compose.ui.geometry.Offset
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs

class MapGestureTest {

    @Test
    fun a_slow_release_or_tap_does_not_fling() {
        assertFalse("zero velocity (a tap) rests", shouldFling(0f, 0f))
        assertFalse("a slow drift rests", shouldFling(40f, 30f)) // |v| = 50 < 220
        assertFalse("a small flick still rests", shouldFling(120f, 90f)) // |v| = 150 < 220
    }

    @Test
    fun a_real_flick_flings() {
        assertTrue("fast horizontal flick", shouldFling(900f, 0f))
        assertTrue("fast diagonal flick", shouldFling(-300f, 400f)) // |v| = 500
    }

    @Test
    fun threshold_is_on_speed_not_a_single_axis() {
        // Just under vs just over the 220 px/s magnitude, off-axis.
        assertFalse(shouldFling(150f, 150f)) // ~212
        assertTrue(shouldFling(160f, 160f)) // ~226
    }

    @Test
    fun small_motion_is_negated_to_prevent_drift() {
        // A sub-threshold release rests (0,0) — no momentum, no drift.
        assertEquals(0f to 0f, flingVelocity(120f, 90f, wasPinch = false))
        // A real flick keeps its velocity.
        assertEquals(900f to 0f, flingVelocity(900f, 0f, wasPinch = false))
    }

    @Test
    fun a_pinch_never_pan_flings() {
        // Even a fast centroid velocity is dropped when a second finger was down,
        // so a zoom doesn't throw the map sideways afterward.
        assertEquals(0f to 0f, flingVelocity(900f, 600f, wasPinch = true))
        assertEquals(0f to 0f, flingVelocity(0f, 0f, wasPinch = true))
    }

    @Test
    fun fling_gate_honours_a_custom_density_scaled_threshold() {
        // The detector passes a higher, density-scaled px/s gate (≈160 dp/s). Verify the
        // pure gate respects whatever threshold it's given: a release just under rests,
        // just over flings — so "only real-velocity swipes drift".
        assertFalse("just under the gate rests", shouldFling(400f, 0f, minVelocity = 440f))
        assertTrue("a real flick over the gate flings", shouldFling(500f, 0f, minVelocity = 440f))
        assertEquals(0f to 0f, flingVelocity(300f, 0f, wasPinch = false, minVelocity = 440f))
        assertEquals(900f to 0f, flingVelocity(900f, 0f, wasPinch = false, minVelocity = 440f))
    }

    @Test
    fun two_finger_angle_and_delta_track_a_twist() {
        // Horizontal pair → 0°; rotate the far finger up-left a touch → angle grows
        // (screen-y is down, so clockwise is positive). Delta wraps the seam.
        val flat = twoFingerAngleDeg(Offset(0f, 0f), Offset(100f, 0f))
        assertEquals(0f, flat, 1e-3f)
        val tilted = twoFingerAngleDeg(Offset(0f, 0f), Offset(100f, 100f)) // 45° (y down)
        assertEquals(45f, tilted, 1e-3f)
        // wrapDeltaDeg keeps a frame-to-frame delta in (-180, 180].
        assertEquals(10f, wrapDeltaDeg(10f), 1e-3f)
        assertEquals(-10f, wrapDeltaDeg(350f), 1e-3f)
        // Crossing the ±180 seam (170° → -175°, raw delta 345°) is a small -15°, not a ~360 jump.
        assertEquals(-15f, wrapDeltaDeg(345f), 1e-3f)
        assertTrue("near-seam delta stays small", abs(wrapDeltaDeg(170f - (-175f))) < 20f)
    }

    @Test
    fun two_finger_gesture_locks_to_a_single_axis() {
        // Below every gate → dead-zone, nothing wins yet.
        assertNull(lockTwoFingerAxis(zoomN = 0.9f, rotateN = 0.5f, tiltN = 0.2f))
        // A clean pinch (zoom past its gate, little twist/tilt) → Zoom.
        assertEquals(TwoFingerAxis.Zoom, lockTwoFingerAxis(zoomN = 1.4f, rotateN = 0.3f, tiltN = 0.1f))
        // A clean twist → Rotate, even if a little incidental zoom crept in.
        assertEquals(TwoFingerAxis.Rotate, lockTwoFingerAxis(zoomN = 0.8f, rotateN = 1.6f, tiltN = 0.2f))
        // The most-progressed axis wins when several have crossed.
        assertEquals(TwoFingerAxis.Tilt, lockTwoFingerAxis(zoomN = 1.1f, rotateN = 1.2f, tiltN = 2.0f))
    }

    @Test
    fun zoom_tracker_reports_levels_per_second() {
        // Spread doubles (×2 = +1 zoom level) over 100 ms → ~10 levels/s.
        val tracker = ZoomVelocityTracker()
        tracker.addRatio(0L, 1f)
        tracker.addRatio(50L, 1.41421f) // ×√2 = +0.5 levels
        tracker.addRatio(100L, 1.41421f) // ×√2 = +0.5 levels (total +1 over 100 ms)
        assertEquals(10f, tracker.velocity(), 0.3f)
    }

    @Test
    fun zoom_tracker_zero_until_two_samples_and_after_reset() {
        val tracker = ZoomVelocityTracker()
        assertEquals(0f, tracker.velocity(), 0f)
        tracker.addRatio(0L, 2f)
        assertEquals("one sample is not yet a rate", 0f, tracker.velocity(), 0f)
        tracker.addRatio(50L, 2f)
        assertTrue("two samples give a rate", tracker.velocity() > 0f)
        tracker.reset()
        assertEquals("reset clears the window", 0f, tracker.velocity(), 0f)
    }

    @Test
    fun a_slow_pinch_release_does_not_zoom_fling() {
        assertFalse("imperceptible zoom rate rests", shouldZoomFling(0.2f))
        assertTrue("a brisk pinch coasts", shouldZoomFling(3.0f))
        assertTrue("zoom-out coasts too (sign-agnostic)", shouldZoomFling(-3.0f))
    }
}
