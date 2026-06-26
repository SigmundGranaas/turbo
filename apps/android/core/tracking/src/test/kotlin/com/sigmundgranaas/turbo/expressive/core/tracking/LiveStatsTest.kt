package com.sigmundgranaas.turbo.expressive.core.tracking

import com.sigmundgranaas.turbo.expressive.core.geo.RouteProgress
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class LiveStatsTest {

    @Test
    fun `recording session maps distance, speed, ascent and energy`() {
        val session = RecordingSession(
            active = true,
            distanceM = 6_200.0,
            elapsedSec = 2_892,
            elevations = listOf(400.0, 500.0, 480.0, 812.0),
            speedMps = 2.5,
            maxSpeedMps = 3.9,
        )
        val stats = LiveStats.of(session)

        assertEquals(LiveMode.Recording, stats.mode)
        assertEquals(6_200.0, stats.distanceM, 1e-6)
        assertEquals(2.5, stats.speedMps!!, 1e-6)
        assertEquals(3.9, stats.maxSpeedMps!!, 1e-6)
        // gain = (500-400)+(812-480) = 432; descent = 20; altitude = last = 812.
        assertEquals(432.0, stats.ascentM!!, 1e-6)
        assertEquals(20.0, stats.descentM!!, 1e-6)
        assertEquals(812.0, stats.altitudeM!!, 1e-6)
        assertTrue(stats.kcal > 0)
    }

    @Test
    fun `following session maps remaining distance, fraction, eta and ascent-to-go`() {
        val plan = RoutePlan(
            distanceM = 10_300.0, durationS = 7_200.0, ascentM = 460.0,
            onTrailPct = 90.0, surfaces = emptyMap(),
            geometry = listOf(LatLng(69.0, 18.0), LatLng(69.05, 18.0)),
        )
        val session = FollowSession(
            active = true,
            plan = plan,
            progress = RouteProgress(fraction = 0.62, distanceRemainingM = 4_100.0, etaSeconds = 2_520, offRoute = false, arrived = false),
            speedMps = 2.5,
        )
        val stats = LiveStats.of(session)

        assertEquals(LiveMode.Following, stats.mode)
        assertEquals(4_100.0, stats.distanceRemainingM!!, 1e-6)
        assertEquals(10_300.0, stats.routeDistanceM!!, 1e-6)
        assertEquals(0.62, stats.fraction!!, 1e-6)
        assertEquals(2_520, stats.etaSeconds)
        // ascent-to-go = total * (1 - fraction) = 460 * 0.38 = 174.8.
        assertEquals(174.8, stats.ascentRemainingM!!, 1e-3)
    }

    @Test
    fun `following with no fix yet falls back to the planned totals`() {
        val plan = RoutePlan(
            distanceM = 8_000.0, durationS = 5_400.0, ascentM = 300.0,
            onTrailPct = 80.0, surfaces = emptyMap(),
            geometry = listOf(LatLng(69.0, 18.0), LatLng(69.05, 18.0)),
        )
        val stats = LiveStats.of(FollowSession(active = true, plan = plan))
        assertEquals(8_000.0, stats.distanceRemainingM!!, 1e-6)
        assertEquals(300.0, stats.ascentRemainingM!!, 1e-6)
        assertEquals(null, stats.fraction)
    }
}
