package com.sigmundgranaas.turbo.expressive.feature.offline

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.core.map.OfflineTileManager
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.LocationDescription
import com.sigmundgranaas.turbo.expressive.domain.OfflineEstimate
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.OfflineStatus
import com.sigmundgranaas.turbo.expressive.domain.PlaceQualifier
import com.sigmundgranaas.turbo.expressive.feature.map.MainDispatcherRule
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

private class FakeOfflineTileManager(initial: List<OfflineRegionInfo> = emptyList()) : OfflineTileManager {
    private val flow = MutableStateFlow(initial)
    override val regions: StateFlow<List<OfflineRegionInfo>> = flow

    var refreshCount = 0
    var lastDownload: DownloadSpec? = null
    val deleted = mutableListOf<Long>()
    val retried = mutableListOf<Long>()

    override fun refresh() { refreshCount++ }
    override fun download(spec: DownloadSpec) { lastDownload = spec }
    override fun retry(id: Long) { retried += id }
    override fun pause(id: Long) = Unit
    override fun resume(id: Long) = Unit
    override fun setNetworkAllowed(allowed: Boolean) = Unit
    override fun estimate(spec: DownloadSpec) = OfflineEstimate(tiles = 0, bytes = 0)
    override fun delete(id: Long) {
        deleted += id
        flow.value = flow.value.filterNot { it.id == id }
    }
}

/** Reverse-geocode stub: names the point, or returns Failure to exercise the fallback. */
private class FakeReverseGeocode(private val title: String?) : ReverseGeocodeRepository {
    override suspend fun describe(point: LatLng): Outcome<LocationDescription> =
        title?.let { Outcome.Success(LocationDescription(title = it, qualifier = PlaceQualifier.On)) }
            ?: Outcome.Failure(IllegalStateException("none"))
}

@OptIn(ExperimentalCoroutinesApi::class)
class OfflineViewModelTest {

    @get:Rule
    val mainRule = MainDispatcherRule()

    private val bounds = GeoBounds(south = 69.0, west = 18.0, north = 69.1, east = 18.2)
    private val centre = LatLng(69.05, 18.1)

    @Test
    fun `init refreshes the region list`() {
        val manager = FakeOfflineTileManager()
        OfflineViewModel(manager, FakeReverseGeocode("Tromsø"))
        assertEquals(1, manager.refreshCount)
    }

    @Test
    fun `download names the region by the reverse-geocoded place and spans zooms`() = runTest(mainRule.dispatcher) {
        val manager = FakeOfflineTileManager()
        OfflineViewModel(manager, FakeReverseGeocode("Storfjellet")).download(centre, BaseLayer.Norgeskart, bounds, fromZoom = 10.4)
        advanceUntilIdle()

        val d = manager.lastDownload!!
        assertEquals("Storfjellet", d.name) // place name, not coordinates
        assertEquals(BaseLayer.Norgeskart, d.base)
        assertEquals(10.0, d.minZoom, 1e-9)
        assertEquals(14.0, d.maxZoom, 1e-9)
    }

    @Test
    fun `download falls back to coordinates when reverse-geocode has no name`() = runTest(mainRule.dispatcher) {
        val manager = FakeOfflineTileManager()
        OfflineViewModel(manager, FakeReverseGeocode(null)).download(centre, BaseLayer.Norgeskart, bounds, fromZoom = 10.0)
        advanceUntilIdle()
        // Coordinate fallback contains the cardinal markers from formatCoords.
        assertTrue("expected a coordinate name, got ${manager.lastDownload?.name}", manager.lastDownload!!.name.contains("N"))
    }

    @Test
    fun `download clamps the zoom span to the max level`() = runTest(mainRule.dispatcher) {
        val manager = FakeOfflineTileManager()
        OfflineViewModel(manager, FakeReverseGeocode("X")).download(centre, BaseLayer.Osm, bounds, fromZoom = 15.0)
        advanceUntilIdle()
        val d = manager.lastDownload!!
        assertEquals(15.0, d.minZoom, 1e-9)
        assertEquals(16.0, d.maxZoom, 1e-9)
    }

    @Test
    fun `delete forwards to the manager and removes the region`() {
        val manager = FakeOfflineTileManager(
            listOf(OfflineRegionInfo(id = 7, name = "A", status = OfflineStatus.Complete, progress = 1f, sizeBytes = 1_000)),
        )
        val vm = OfflineViewModel(manager, FakeReverseGeocode("A"))
        vm.delete(7)
        assertTrue(manager.deleted.contains(7))
        assertTrue(vm.regions.value.isEmpty())
    }
}
