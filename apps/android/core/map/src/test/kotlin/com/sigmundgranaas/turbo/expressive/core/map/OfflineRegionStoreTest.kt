package com.sigmundgranaas.turbo.expressive.core.map

import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class OfflineRegionStoreTest {

    @get:Rule val tmp = TemporaryFolder()

    private fun region(id: Long, name: String = "Bodø — Sjunkhatten") = OfflineRegionInfo(
        id = id,
        name = name,
        status = OfflineStatus.Complete,
        progress = 1f,
        sizeBytes = 1_234_567L,
        tileCount = 42L,
        base = BaseLayer.Norgeskart,
        overlays = setOf(OverlayId.Trails),
        bounds = GeoBounds(south = 67.2, west = 14.9, north = 67.3, east = 15.1),
        minZoom = 8.0,
        maxZoom = 14.0,
        createdAtEpochMs = 1_700_000_000_000L,
        errorReason = null,
    )

    @Test
    fun `region round-trips through disk`() {
        val store = OfflineRegionStore(tmp.newFolder("regions"))
        val original = region(7L)
        store.save(original)
        assertEquals(original, OfflineRegionStore(tmp.root.resolve("regions")).loadAll().single())
    }

    @Test
    fun `failed region preserves its error reason`() {
        val dir = tmp.newFolder("regions")
        val store = OfflineRegionStore(dir)
        store.save(region(3L).copy(status = OfflineStatus.Failed, errorReason = "Area too large"))
        val loaded = OfflineRegionStore(dir).loadAll().single()
        assertEquals(OfflineStatus.Failed, loaded.status)
        assertEquals("Area too large", loaded.errorReason)
    }

    @Test
    fun `delete forgets a region`() {
        val dir = tmp.newFolder("regions")
        val store = OfflineRegionStore(dir)
        store.save(region(1L, "A"))
        store.save(region(2L, "B"))
        store.delete(1L)
        val ids = OfflineRegionStore(dir).loadAll().map { it.id }
        assertEquals(listOf(2L), ids)
    }

    @Test
    fun `loadAll is newest-first`() {
        val dir = tmp.newFolder("regions")
        val store = OfflineRegionStore(dir)
        store.save(region(1L, "old").copy(createdAtEpochMs = 1000L))
        store.save(region(2L, "new").copy(createdAtEpochMs = 2000L))
        assertEquals(listOf(2L, 1L), OfflineRegionStore(dir).loadAll().map { it.id })
        assertTrue(true)
    }
}
