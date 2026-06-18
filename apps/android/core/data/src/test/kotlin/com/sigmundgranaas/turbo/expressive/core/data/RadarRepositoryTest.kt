package com.sigmundgranaas.turbo.expressive.core.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Pure grid-resampling logic behind [HttpRadarRepository] (no network). */
class RadarRepositoryTest {

    /** Build a `cols*rows` row-major sample set, one frame, from a (c,r) fn. */
    private fun samples(
        cols: Int,
        rows: Int,
        coverage: (c: Int, r: Int) -> Double,
        precip: (c: Int, r: Int) -> Double = { _, _ -> 0.0 },
    ): List<List<SampleStep>> = buildList {
        for (r in 0 until rows) {
            for (c in 0 until cols) {
                add(listOf(SampleStep(epochMillis = 1000L, coverage = coverage(c, r), precipMm = precip(c, r))))
            }
        }
    }

    private fun ByteArray.u(i: Int) = this[i].toInt() and 0xff

    @Test
    fun uniform_field_maps_to_uniform_bytes() {
        val frames = buildFrames(
            samples(3, 3, coverage = { _, _ -> 0.5 }, precip = { _, _ -> PRECIP_FULL_MM / 2 }),
            cols = 3, rows = 3, gridW = 8, gridH = 8, frames = 1,
        )
        assertEquals(1, frames.size)
        val f = frames[0]
        // 0.5 coverage → ~127; half the full-precip rate → ~127.
        assertTrue(f.coverage.all { (it.toInt() and 0xff) in 125..129 })
        assertTrue(f.precip.all { (it.toInt() and 0xff) in 125..129 })
        assertEquals(1000L, f.epochMillis)
    }

    @Test
    fun coverage_interpolates_left_to_right() {
        // Linear west→east ramp across the sample columns (1.0 → 0.5 → 0.0).
        val frames = buildFrames(
            samples(3, 3, coverage = { c, _ -> 1.0 - c.toDouble() / 2.0 }),
            cols = 3, rows = 3, gridW = 16, gridH = 4, frames = 1,
        )
        val cov = frames[0].coverage
        val row = 0
        val left = cov.u(row * 16 + 0)
        val mid = cov.u(row * 16 + 8)
        val right = cov.u(row * 16 + 15)
        assertTrue("left>mid>right, got $left/$mid/$right", left > mid && mid > right)
        assertTrue("left near full", left > 200)
        assertTrue("right near zero", right < 60)
    }

    @Test
    fun frame_count_clamped_to_shortest_series() {
        // One point has only 1 step; requesting 3 yields 1.
        val perPoint = listOf(
            listOf(SampleStep(1L, 0.3, 0.0), SampleStep(2L, 0.3, 0.0), SampleStep(3L, 0.3, 0.0)),
            listOf(SampleStep(1L, 0.3, 0.0)), // short
            listOf(SampleStep(1L, 0.3, 0.0), SampleStep(2L, 0.3, 0.0)),
            listOf(SampleStep(1L, 0.3, 0.0)),
        )
        val frames = buildFrames(perPoint, cols = 2, rows = 2, gridW = 4, gridH = 4, frames = 3)
        assertEquals(1, frames.size)
    }
}
