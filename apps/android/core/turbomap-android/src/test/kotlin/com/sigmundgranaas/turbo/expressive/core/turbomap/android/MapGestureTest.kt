package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MapGestureTest {

    @Test
    fun a_slow_release_or_tap_does_not_fling() {
        assertFalse("zero velocity (a tap) rests", shouldFling(0f, 0f))
        assertFalse("a slow drift rests", shouldFling(40f, 30f)) // |v| = 50 < 120
    }

    @Test
    fun a_real_flick_flings() {
        assertTrue("fast horizontal flick", shouldFling(900f, 0f))
        assertTrue("fast diagonal flick", shouldFling(-300f, 400f)) // |v| = 500
    }

    @Test
    fun threshold_is_on_speed_not_a_single_axis() {
        // Just under vs just over the 120 px/s magnitude, off-axis.
        assertFalse(shouldFling(80f, 80f)) // ~113
        assertTrue(shouldFling(90f, 90f)) // ~127
    }
}
