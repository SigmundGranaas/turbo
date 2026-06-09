package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class OfflineRegionMetadataTest {

    @Test
    fun `round-trips every field including a unicode name with a delimiter`() {
        val meta = OfflineRegionMetadata.Meta(
            name = "Tromsø, Nord-Norge",
            base = BaseLayer.Satellite,
            overlays = setOf(OverlayId.Avalanche, OverlayId.Trails),
            bounds = GeoBounds(south = 69.6, west = 18.9, north = 69.7, east = 19.1),
            minZoom = 8.0,
            maxZoom = 14.0,
            createdAtEpochMs = 1_700_000_000_000L,
        )
        assertEquals(meta, OfflineRegionMetadata.decode(OfflineRegionMetadata.encode(meta)))
    }

    @Test
    fun `a legacy bare-name blob decodes to a name with defaults`() {
        val decoded = OfflineRegionMetadata.decode("Lofoten".toByteArray())
        assertEquals("Lofoten", decoded?.name)
        assertEquals(BaseLayer.Norgeskart, decoded?.base)
        assertNull(decoded?.bounds)
    }

    @Test
    fun `null or empty input decodes to null`() {
        assertNull(OfflineRegionMetadata.decode(null))
        assertNull(OfflineRegionMetadata.decode(ByteArray(0)))
    }
}
