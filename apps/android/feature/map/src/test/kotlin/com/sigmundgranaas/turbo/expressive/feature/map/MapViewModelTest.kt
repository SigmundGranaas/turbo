package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.LocationRepository
import com.sigmundgranaas.turbo.expressive.core.data.LocationSample
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.ReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.LocationDescription
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.PlaceQualifier
import com.sigmundgranaas.turbo.expressive.ui.theme.labelRes
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

private class FakeMarkerRepository : MarkerRepository {
    val markers = MutableStateFlow<List<Marker>>(emptyList())
    override fun observeAll(): Flow<List<Marker>> = markers
    override suspend fun upsert(marker: Marker) { markers.value = markers.value.filterNot { it.id == marker.id } + marker }
    override suspend fun delete(id: String) { markers.value = markers.value.filterNot { it.id == id } }
}

private class FakeLocationRepository(
    var permitted: Boolean = true,
    var enabled: Boolean = true,
) : LocationRepository {
    val fixes = MutableSharedFlow<LatLng>(extraBufferCapacity = 8)
    override fun hasPermission(): Boolean = permitted
    override fun isLocationEnabled(): Boolean = enabled
    override fun samples(): Flow<LocationSample> = fixes.map { LocationSample(it, null) }
}

private class FakeReverseGeocodeRepository(
    var result: LocationDescription? = null,
) : ReverseGeocodeRepository {
    override suspend fun describe(point: LatLng): Outcome<LocationDescription> =
        result?.let { Outcome.Success(it) } ?: Outcome.Failure(IllegalStateException("none"))
}

private class FakeStringProvider : com.sigmundgranaas.turbo.expressive.core.common.StringProvider {
    override fun get(id: Int): String = "s$id"
    override fun get(id: Int, vararg formatArgs: Any): String = "s$id:" + formatArgs.joinToString()
}

@OptIn(ExperimentalCoroutinesApi::class)
class MapViewModelTest {

    @get:Rule
    val mainRule = MainDispatcherRule()

    @Test
    fun `addMarker persists and surfaces in state`() = runTest(mainRule.dispatcher) {
        val markers = FakeMarkerRepository()
        val vm = MapViewModel(markers, FakeLocationRepository(), FakeReverseGeocodeRepository(), FakeStringProvider())
        vm.addMarker("Camp", ActivityKindId.Camping, LatLng(69.0, 18.0))
        advanceUntilIdle()

        assertEquals(1, vm.state.value.markers.size)
        assertEquals("Camp", vm.state.value.markers[0].name)
        assertEquals(ActivityKindId.Camping, vm.state.value.markers[0].kind)
    }

    @Test
    fun `addMarker keeps the chosen colour and notes`() = runTest(mainRule.dispatcher) {
        val markers = FakeMarkerRepository()
        val vm = MapViewModel(markers, FakeLocationRepository(), FakeReverseGeocodeRepository(), FakeStringProvider())
        vm.addMarker("Hut", ActivityKindId.Cabin, LatLng(69.0, 18.0), colorArgb = 0xFF1A73E8, notes = "spring water nearby")
        advanceUntilIdle()

        val m = vm.state.value.markers.single()
        assertEquals(0xFF1A73E8, m.colorArgb)
        assertEquals("spring water nearby", m.notes)
    }

    @Test
    fun `updateMarker replaces the row in place`() = runTest(mainRule.dispatcher) {
        val markers = FakeMarkerRepository()
        markers.markers.value = listOf(Marker("m1", "Old", ActivityKindId.Cabin, LatLng(69.0, 18.0)))
        val vm = MapViewModel(markers, FakeLocationRepository(), FakeReverseGeocodeRepository(), FakeStringProvider())
        advanceUntilIdle()

        vm.updateMarker(
            Marker("m1", "New", ActivityKindId.Fishing, LatLng(69.0, 18.0), colorArgb = 0xFFE0432B, notes = "good perch"),
        )
        advanceUntilIdle()

        val m = vm.state.value.markers.single()
        assertEquals("m1", m.id)
        assertEquals("New", m.name)
        assertEquals(ActivityKindId.Fishing, m.kind)
        assertEquals("good perch", m.notes)
    }

    @Test
    fun `blank name falls back to the kind label`() = runTest(mainRule.dispatcher) {
        val markers = FakeMarkerRepository()
        val strings = FakeStringProvider()
        val vm = MapViewModel(markers, FakeLocationRepository(), FakeReverseGeocodeRepository(), strings)
        vm.addMarker("  ", ActivityKindId.Cabin, LatLng(69.0, 18.0))
        advanceUntilIdle()
        assertEquals(strings.get(ActivityKindId.Cabin.labelRes), vm.state.value.markers[0].name)
    }

    @Test
    fun `deleteMarker removes it`() = runTest(mainRule.dispatcher) {
        val markers = FakeMarkerRepository()
        markers.markers.value = listOf(Marker("m1", "A", ActivityKindId.Cabin, LatLng(69.0, 18.0)))
        val vm = MapViewModel(markers, FakeLocationRepository(), FakeReverseGeocodeRepository(), FakeStringProvider())
        advanceUntilIdle()
        assertEquals(1, vm.state.value.markers.size)

        vm.deleteMarker("m1")
        advanceUntilIdle()
        assertTrue(vm.state.value.markers.isEmpty())
    }

    @Test
    fun `enableLocation streams fixes into state`() = runTest(mainRule.dispatcher) {
        val location = FakeLocationRepository(permitted = true)
        val vm = MapViewModel(FakeMarkerRepository(), location, FakeReverseGeocodeRepository(), FakeStringProvider())
        vm.enableLocation()
        runCurrent()
        location.fixes.emit(LatLng(69.65, 18.95))
        runCurrent()

        assertEquals(LatLng(69.65, 18.95), vm.state.value.userLocation)
    }

    @Test
    fun `enableLocation is a no-op without permission`() = runTest(mainRule.dispatcher) {
        val location = FakeLocationRepository(permitted = false)
        val vm = MapViewModel(FakeMarkerRepository(), location, FakeReverseGeocodeRepository(), FakeStringProvider())
        vm.enableLocation()
        runCurrent()
        location.fixes.emit(LatLng(1.0, 2.0))
        runCurrent()
        assertEquals(null, vm.state.value.userLocation)
    }

    @Test
    fun `enableLocation marks locating until the first fix arrives`() = runTest(mainRule.dispatcher) {
        val location = FakeLocationRepository(permitted = true)
        val vm = MapViewModel(FakeMarkerRepository(), location, FakeReverseGeocodeRepository(), FakeStringProvider())
        vm.enableLocation()
        runCurrent()
        assertTrue(vm.state.value.locating)
        location.fixes.emit(LatLng(69.65, 18.95))
        runCurrent()
        assertTrue(!vm.state.value.locating)
        assertEquals(null, vm.state.value.locationNotice)
    }

    @Test
    fun `beginInitialLocate flags ServicesOff when location is disabled`() = runTest(mainRule.dispatcher) {
        val location = FakeLocationRepository(permitted = true, enabled = false)
        val vm = MapViewModel(FakeMarkerRepository(), location, FakeReverseGeocodeRepository(), FakeStringProvider())
        vm.beginInitialLocate()
        runCurrent()
        assertEquals(LocationNotice.ServicesOff, vm.state.value.locationNotice)
        assertTrue(!vm.state.value.locating)
    }

    @Test
    fun `beginInitialLocate times out to a notice when no fix arrives`() = runTest(mainRule.dispatcher) {
        val location = FakeLocationRepository(permitted = true, enabled = true)
        val vm = MapViewModel(FakeMarkerRepository(), location, FakeReverseGeocodeRepository(), FakeStringProvider())
        vm.beginInitialLocate()
        runCurrent()
        assertTrue(vm.state.value.locating)
        advanceTimeBy(13_000)
        runCurrent()
        assertEquals(LocationNotice.Timeout, vm.state.value.locationNotice)
        assertTrue(!vm.state.value.locating)
    }

    @Test
    fun `a late fix after timeout still recentres and clears the notice`() = runTest(mainRule.dispatcher) {
        val location = FakeLocationRepository(permitted = true, enabled = true)
        val vm = MapViewModel(FakeMarkerRepository(), location, FakeReverseGeocodeRepository(), FakeStringProvider())
        vm.beginInitialLocate()
        advanceTimeBy(13_000)
        runCurrent()
        assertEquals(LocationNotice.Timeout, vm.state.value.locationNotice)
        location.fixes.emit(LatLng(60.0, 10.0))
        runCurrent()
        assertEquals(LatLng(60.0, 10.0), vm.state.value.userLocation)
        assertEquals(null, vm.state.value.locationNotice)
    }

    @Test
    fun `describePoint resolves a label and clearPointDescription resets it`() = runTest(mainRule.dispatcher) {
        val geocoder = FakeReverseGeocodeRepository(
            LocationDescription("Galdhøpiggen", PlaceQualifier.On, "fjelltopp", 2469.0),
        )
        val vm = MapViewModel(FakeMarkerRepository(), FakeLocationRepository(), geocoder, FakeStringProvider())
        vm.describePoint(LatLng(61.636, 8.313))
        advanceUntilIdle()
        assertEquals("Galdhøpiggen", vm.pointDescription.value?.title)
        assertEquals("On Galdhøpiggen", vm.pointDescription.value?.label)

        vm.clearPointDescription()
        assertEquals(null, vm.pointDescription.value)
    }
}
