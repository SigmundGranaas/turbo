package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

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
