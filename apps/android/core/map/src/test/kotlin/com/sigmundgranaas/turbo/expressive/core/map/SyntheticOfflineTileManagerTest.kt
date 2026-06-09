package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SyntheticOfflineTileManagerTest {

    private val bounds = GeoBounds(south = 69.6, west = 18.9, north = 69.7, east = 19.1)
    private fun spec(name: String, b: GeoBounds = bounds, min: Double = 8.0, max: Double = 15.0) =
        DownloadSpec(name = name, base = BaseLayer.Norgeskart, bounds = b, minZoom = min, maxZoom = max)

    @Test
    fun `download adds a complete region carrying its metadata`() {
        val m = SyntheticOfflineTileManager()
        m.download(spec("Tromsø area"))
        val r = m.regions.value.single()
        assertEquals("Tromsø area", r.name)
        assertEquals(OfflineStatus.Complete, r.status)
        assertEquals(1f, r.progress)
        assertTrue("size should be non-trivial", r.sizeBytes > 0)
        assertTrue("tiles should be counted", r.tileCount > 0)
        assertEquals(BaseLayer.Norgeskart, r.base)
        assertEquals(bounds, r.bounds)
    }

    @Test
    fun `an over-limit area fails and retry completes it`() {
        val m = SyntheticOfflineTileManager()
        val huge = GeoBounds(south = 60.0, west = 5.0, north = 72.0, east = 25.0)
        m.download(spec("Whole country", b = huge))
        val failed = m.regions.value.single()
        assertEquals(OfflineStatus.Failed, failed.status)
        assertTrue(failed.errorReason != null)

        m.retry(failed.id)
        val retried = m.regions.value.single()
        assertEquals(OfflineStatus.Complete, retried.status)
        assertNull(retried.errorReason)
    }

    @Test
    fun `delete removes the region by id`() {
        val m = SyntheticOfflineTileManager()
        m.download(spec("A"))
        m.download(spec("B"))
        val first = m.regions.value.first().id
        m.delete(first)
        assertEquals(listOf("B"), m.regions.value.map { it.name })
    }
}
