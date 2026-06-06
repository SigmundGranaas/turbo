package com.sigmundgranaas.turbo.expressive.ui.map

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ScaleBarTest {

    @Test
    fun `meters per pixel halves as zoom increases by one`() {
        val z10 = ScaleBar.metersPerPixel(60.0, 10.0)
        val z11 = ScaleBar.metersPerPixel(60.0, 11.0)
        assertEquals(z10 / 2.0, z11, z11 * 1e-9)
    }

    @Test
    fun `meters per pixel shrinks toward the poles`() {
        assertTrue(ScaleBar.metersPerPixel(60.0, 10.0) < ScaleBar.metersPerPixel(0.0, 10.0))
    }

    @Test
    fun `chosen bar is a 1-2-5 round number that fits the budget`() {
        val spec = ScaleBar.compute(latitude = 60.0, zoom = 12.0, maxWidthPx = 100f)
        assertTrue("width within budget", spec.widthPx <= 100f)
        // The chosen distance must be one of 1/2/5 × 10ⁿ.
        val mantissa = spec.meters / Math.pow(10.0, Math.floor(Math.log10(spec.meters)))
        assertTrue("mantissa is 1, 2 or 5 (was $mantissa)", mantissa in listOf(1.0, 2.0, 5.0))
    }

    @Test
    fun `labels switch to km at and above 1000 m`() {
        assertTrue(ScaleBar.compute(60.0, 6.0, 120f).label.endsWith("km"))
        assertTrue(ScaleBar.compute(60.0, 18.0, 120f).label.endsWith("m"))
    }
}
