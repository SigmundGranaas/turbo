package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TileMathTest {

    private val small = GeoBounds(south = 60.0, west = 10.0, north = 60.1, east = 10.1)

    @Test
    fun `a point box is a single tile at zoom 0`() {
        val point = GeoBounds(south = 60.0, west = 10.0, north = 60.0, east = 10.0)
        assertEquals(1L, TileMath.tileCount(point, 0.0, 0.0))
    }

    @Test
    fun `tile count grows with the zoom span`() {
        val narrow = TileMath.tileCount(small, 8.0, 10.0)
        val wide = TileMath.tileCount(small, 8.0, 12.0)
        assertTrue("more zoom levels means more tiles", wide > narrow)
    }

    @Test
    fun `estimate scales with the number of sources`() {
        val base = DownloadSpec("x", BaseLayer.Norgeskart, small, 8.0, 12.0)
        val withTwoOverlays = base.copy(overlays = setOf(OverlayId.Trails, OverlayId.Avalanche))
        assertEquals(TileMath.estimate(base).tiles * 3, TileMath.estimate(withTwoOverlays).tiles)
    }

    @Test
    fun `within-limits accepts a small area and rejects a huge span`() {
        assertTrue(TileMath.isWithinLimits(DownloadSpec("ok", BaseLayer.Norgeskart, small, 8.0, 14.0)))
        val huge = GeoBounds(south = 0.0, west = 0.0, north = 50.0, east = 50.0)
        assertFalse(TileMath.isWithinLimits(DownloadSpec("no", BaseLayer.Norgeskart, huge, 8.0, 14.0)))
    }
}
