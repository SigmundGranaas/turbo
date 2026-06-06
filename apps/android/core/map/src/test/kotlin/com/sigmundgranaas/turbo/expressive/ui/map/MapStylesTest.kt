package com.sigmundgranaas.turbo.expressive.ui.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MapStylesTest {

    @Test
    fun `no overlays yields just the base raster`() {
        val json = MapStyles.styleJson(BaseLayer.Norgeskart)
        assertTrue("base source present", json.contains("\"norgeskart\""))
        assertFalse("no overlay sources", json.contains("\"ov_"))
    }

    @Test
    fun `trails plus avalanche composite both overlay rasters in order`() {
        val json = MapStyles.styleJson(BaseLayer.Norgeskart, setOf(OverlayId.Trails, OverlayId.Avalanche))
        assertTrue("trails source", json.contains("\"ov_Trails\""))
        assertTrue("avalanche source", json.contains("\"ov_Avalanche\""))
        assertTrue("waymarked tiles wired", json.contains("waymarkedtrails.org"))
        assertTrue("NVE Bratthet tiles wired", json.contains("Bratthet_2024"))
        // Overlay layers sit above the base layer.
        assertTrue(json.indexOf("\"id\": \"ov_Trails\"") > json.indexOf("\"id\": \"norgeskart\""))
    }

    @Test
    fun `wind and waves have no tile source so produce no layer`() {
        val json = MapStyles.styleJson(BaseLayer.Osm, setOf(OverlayId.Wind, OverlayId.Waves))
        assertFalse(json.contains("ov_Wind"))
        assertFalse(json.contains("ov_Waves"))
    }

    @Test
    fun `renderableOverlays is exactly trails and avalanche`() {
        assertTrue(MapStyles.renderableOverlays.contains(OverlayId.Trails))
        assertTrue(MapStyles.renderableOverlays.contains(OverlayId.Avalanche))
        assertFalse(MapStyles.renderableOverlays.contains(OverlayId.Wind))
        assertFalse(MapStyles.renderableOverlays.contains(OverlayId.Waves))
    }
}
