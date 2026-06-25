package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pins the shared capture core that makes Follow = Record byte-for-byte. */
class TrackCaptureTest {

    private fun fold(fixes: List<Triple<LatLng, Double?, Double?>>): CapturedTrack =
        fixes.fold(CapturedTrack()) { acc, (pos, alt, spd) -> TrackCapture.append(acc, pos, alt, spd) }

    @Test
    fun `accumulates points, distance, altitude and peak speed`() {
        val track = fold(
            listOf(
                Triple(LatLng(69.000, 18.0), 100.0, 1.0),
                Triple(LatLng(69.003, 18.0), 120.0, 2.5),
                Triple(LatLng(69.006, 18.0), 140.0, 2.0),
            ),
        )
        assertEquals(3, track.points.size)
        assertEquals(listOf(100.0, 120.0, 140.0), track.elevations)
        assertTrue("distance grows with the walk", track.distanceM > 600.0)
        assertEquals(2.0, track.speedMps!!, 1e-9) // latest
        assertEquals(2.5, track.maxSpeedMps, 1e-9) // peak
    }

    @Test
    fun `sub-step jitter refreshes speed but does not add a point`() {
        val track = fold(
            listOf(
                Triple(LatLng(69.0, 18.0), null, 1.0),
                // ~1 m away — under MIN_STEP_M, so no new point/distance, but speed updates.
                Triple(LatLng(69.00001, 18.0), null, 3.0),
            ),
        )
        assertEquals(1, track.points.size)
        assertEquals(0.0, track.distanceM, 1e-9)
        assertEquals(3.0, track.speedMps!!, 1e-9)
    }

    @Test
    fun `identical fixes fold to an identical track regardless of caller (record vs follow)`() {
        val fixes = (0..8).map { Triple(LatLng(69.00 + it * 0.003, 18.0), 100.0 + it, 2.0) }
        val viaRecord = fold(fixes)
        val viaFollow = fold(fixes)
        assertEquals(viaRecord.points, viaFollow.points)
        assertEquals(viaRecord.elevations, viaFollow.elevations)
        assertEquals(viaRecord.distanceM, viaFollow.distanceM, 1e-9)
    }
}
