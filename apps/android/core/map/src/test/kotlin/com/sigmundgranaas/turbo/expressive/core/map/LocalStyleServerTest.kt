package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.net.URL

class LocalStyleServerTest {

    @Test
    fun `serves the base map plus the requested overlays`() {
        val server = LocalStyleServer()
        val url = server.styleUrl(BaseLayer.Norgeskart, setOf(OverlayId.Avalanche))
        assertTrue("overlay encoded in url", url.contains("ov=Avalanche"))

        val body = URL(url).readText()
        assertTrue("base source present", body.contains("\"norgeskart\""))
        assertTrue("avalanche overlay source present", body.contains("\"ov_Avalanche\""))
    }

    @Test
    fun `serves the base map only when no overlays are requested`() {
        val server = LocalStyleServer()
        val body = URL(server.styleUrl(BaseLayer.Osm)).readText()
        assertTrue("base source present", body.contains("\"osm\""))
        assertFalse("no overlay sources", body.contains("\"ov_"))
    }
}
