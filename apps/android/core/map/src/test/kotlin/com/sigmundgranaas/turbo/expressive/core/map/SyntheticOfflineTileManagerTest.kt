package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SyntheticOfflineTileManagerTest {

    private val bounds = GeoBounds(south = 69.6, west = 18.9, north = 69.7, east = 19.1)

    @Test
    fun `download adds a complete region with a plausible size`() {
        val m = SyntheticOfflineTileManager()
        m.download("Tromsø area", BaseLayer.Norgeskart, bounds, minZoom = 8.0, maxZoom = 15.0)
        val r = m.regions.value.single()
        assertEquals("Tromsø area", r.name)
        assertTrue(r.complete)
        assertEquals(1f, r.progress)
        assertTrue("size should be non-trivial", r.sizeBytes >= 2_000_000L)
    }

    @Test
    fun `delete removes the region by id`() {
        val m = SyntheticOfflineTileManager()
        m.download("A", BaseLayer.Norgeskart, bounds, 8.0, 14.0)
        m.download("B", BaseLayer.Norgeskart, bounds, 8.0, 14.0)
        val first = m.regions.value.first().id
        m.delete(first)
        assertEquals(listOf("B"), m.regions.value.map { it.name })
    }
}
