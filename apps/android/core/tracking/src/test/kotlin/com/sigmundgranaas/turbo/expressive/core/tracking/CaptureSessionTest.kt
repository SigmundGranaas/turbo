package com.sigmundgranaas.turbo.expressive.core.tracking

import com.sigmundgranaas.turbo.expressive.domain.LatLng
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The shared pause-buffer capture engine behind both recording and following (US-4). */
class CaptureSessionTest {

    private fun CaptureSession.walk(lat: Double, lng: Double = 18.0, alt: Double? = null, speed: Double? = null) =
        fix(LatLng(lat, lng), alt, speed)

    @Test
    fun `active fixes accumulate into the committed track`() {
        val s = CaptureSession().walk(69.0).walk(69.001) // ~111 m
        assertEquals(2, s.points.size)
        assertFalse(s.paused)
        assertTrue("distance ~111m but was ${s.distanceM}", s.distanceM in 100.0..125.0)
        assertEquals(0.0, s.bufferedDistanceM, 1e-9)
    }

    @Test
    fun `paused fixes buffer instead of touching the track`() {
        val s = CaptureSession().walk(69.0).pause().walk(69.001) // ~111 m while paused
        assertTrue(s.paused)
        assertEquals("track untouched while paused", 1, s.points.size)
        assertTrue("buffered ${s.bufferedDistanceM}", s.bufferedDistanceM > 90.0)
        assertTrue(s.hasBufferedMovement)
    }

    @Test
    fun `resume include stitches the paused walk onto the track`() {
        val s = CaptureSession().walk(69.0).pause().walk(69.001).walk(69.002).resume(include = true)
        assertFalse(s.paused)
        assertEquals(3, s.points.size)
        assertEquals(0.0, s.bufferedDistanceM, 1e-9)
        assertTrue("counts the full paused walk but was ${s.distanceM}", s.distanceM > 200.0)
    }

    @Test
    fun `resume discard drops the walk and lifts the pen so the gap is not counted`() {
        var s = CaptureSession().walk(69.0).pause().walk(69.001).resume(include = false)
        assertEquals(1, s.points.size)
        assertEquals(0.0, s.distanceM, 1e-9)
        // First fix after discard is detached: no gap distance back to the old point.
        s = s.walk(69.001)
        assertEquals(2, s.points.size)
        assertEquals("no gap distance", 0.0, s.distanceM, 1e-9)
        // …then normal accumulation resumes.
        s = s.walk(69.002)
        assertTrue("walks from the new point count", s.distanceM > 90.0)
    }

    @Test
    fun `restore splits a paused draft back into track and buffer`() {
        val pts = listOf(LatLng(69.0, 18.0), LatLng(69.001, 18.0), LatLng(69.002, 18.0))
        val s = CaptureSession.restore(pts, listOf(null, null, null), pausedFromIndex = 2)
        assertTrue(s.paused)
        assertEquals("track restored without the buffer", 2, s.points.size)
        assertTrue("buffer restored ${s.bufferedDistanceM}", s.bufferedDistanceM > 90.0)
    }

    @Test
    fun `restore with the no-pause sentinel keeps the whole track committed`() {
        val pts = listOf(LatLng(69.0, 18.0), LatLng(69.001, 18.0))
        val s = CaptureSession.restore(pts, listOf(10.0, 25.0), pausedFromIndex = -1)
        assertFalse(s.paused)
        assertEquals(2, s.points.size)
        assertTrue(s.distanceM > 90.0)
    }

    @Test
    fun `speed always refreshes, even on a sub-min-step jitter fix`() {
        val s = CaptureSession()
            .walk(69.0, speed = 1.2)
            .walk(69.001, speed = 3.9)
            .walk(69.001002, speed = 2.0) // jitter (< min step) but updates speed
        assertEquals(2.0, s.speedMps!!, 1e-6)
        assertEquals(3.9, s.maxSpeedMps, 1e-6)
        assertEquals(2, s.points.size)
    }
}
