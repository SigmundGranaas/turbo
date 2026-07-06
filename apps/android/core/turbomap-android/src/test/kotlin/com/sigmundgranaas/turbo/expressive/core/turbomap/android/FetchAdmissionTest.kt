package com.sigmundgranaas.turbo.expressive.core.turbomap.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The host's plan-start admission (plan P5.1). The ENGINE owns planning —
 * these tests pin the only two policies the host keeps: lane capacity and
 * failure backoff. A declined start is reported cancelled so the engine
 * re-issues it; admission must therefore be a pure, deterministic gate.
 */
class FetchAdmissionTest {
    @Test
    fun admits_when_lane_has_room_and_no_backoff() {
        assertTrue(
            admitFetch(
                key = "base/10/1/2",
                laneUsed = 3,
                laneCap = 8,
                alreadyInFlight = false,
                retryAt = emptyMap(),
                now = 1_000L,
            ),
        )
    }

    @Test
    fun declines_a_full_lane() {
        assertFalse(
            admitFetch(
                key = "base/10/1/2",
                laneUsed = 8,
                laneCap = 8,
                alreadyInFlight = false,
                retryAt = emptyMap(),
                now = 1_000L,
            ),
        )
    }

    @Test
    fun declines_inside_the_backoff_window_and_admits_after() {
        val backoff = mapOf("base/10/1/2" to 2_000L)
        assertFalse(
            admitFetch(
                key = "base/10/1/2",
                laneUsed = 0,
                laneCap = 8,
                alreadyInFlight = false,
                retryAt = backoff,
                now = 1_500L,
            ),
        )
        assertTrue(
            admitFetch(
                key = "base/10/1/2",
                laneUsed = 0,
                laneCap = 8,
                alreadyInFlight = false,
                retryAt = backoff,
                now = 2_500L,
            ),
        )
    }

    @Test
    fun declines_a_duplicate_of_an_in_flight_fetch() {
        assertFalse(
            admitFetch(
                key = "base/10/1/2",
                laneUsed = 1,
                laneCap = 8,
                alreadyInFlight = true,
                retryAt = emptyMap(),
                now = 1_000L,
            ),
        )
    }
}
