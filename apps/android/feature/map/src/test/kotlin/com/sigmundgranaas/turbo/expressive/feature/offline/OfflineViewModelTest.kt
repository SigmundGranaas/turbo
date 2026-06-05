package com.sigmundgranaas.turbo.expressive.feature.offline

import com.sigmundgranaas.turbo.expressive.core.map.OfflineTileManager
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

private class FakeOfflineTileManager(initial: List<OfflineRegionInfo> = emptyList()) : OfflineTileManager {
    data class Download(val name: String, val base: BaseLayer, val bounds: GeoBounds, val minZoom: Double, val maxZoom: Double)

    private val flow = MutableStateFlow(initial)
    override val regions: StateFlow<List<OfflineRegionInfo>> = flow

    var refreshCount = 0
    var lastDownload: Download? = null
    val deleted = mutableListOf<Long>()

    override fun refresh() { refreshCount++ }
    override fun download(name: String, base: BaseLayer, bounds: GeoBounds, minZoom: Double, maxZoom: Double) {
        lastDownload = Download(name, base, bounds, minZoom, maxZoom)
    }
    override fun delete(id: Long) {
        deleted += id
        flow.value = flow.value.filterNot { it.id == id }
    }
}

class OfflineViewModelTest {

    private val bounds = GeoBounds(south = 69.0, west = 18.0, north = 69.1, east = 18.2)

    @Test
    fun `init refreshes the region list`() {
        val manager = FakeOfflineTileManager()
        OfflineViewModel(manager)
        assertEquals(1, manager.refreshCount)
    }

    @Test
    fun `download spans a few zoom levels around the camera`() {
        val manager = FakeOfflineTileManager()
        val vm = OfflineViewModel(manager)

        vm.download("Tromsø", BaseLayer.Norgeskart, bounds, fromZoom = 10.4)

        val d = manager.lastDownload!!
        assertEquals("Tromsø", d.name)
        assertEquals(BaseLayer.Norgeskart, d.base)
        assertEquals(bounds, d.bounds)
        assertEquals(10.0, d.minZoom, 1e-9)
        assertEquals(14.0, d.maxZoom, 1e-9)
    }

    @Test
    fun `download clamps the zoom span to the max level`() {
        val manager = FakeOfflineTileManager()
        OfflineViewModel(manager).download("High", BaseLayer.Osm, bounds, fromZoom = 15.0)
        val d = manager.lastDownload!!
        assertEquals(15.0, d.minZoom, 1e-9)
        assertEquals(16.0, d.maxZoom, 1e-9)
    }

    @Test
    fun `delete forwards to the manager and removes the region`() {
        val manager = FakeOfflineTileManager(
            listOf(OfflineRegionInfo(id = 7, name = "A", complete = true, progress = 1f, sizeBytes = 1_000)),
        )
        val vm = OfflineViewModel(manager)
        vm.delete(7)
        assertTrue(manager.deleted.contains(7))
        assertTrue(vm.regions.value.isEmpty())
    }
}
