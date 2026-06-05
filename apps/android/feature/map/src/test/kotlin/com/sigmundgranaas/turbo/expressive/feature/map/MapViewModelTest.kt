package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.core.data.LocationRepository
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
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

private class FakeLocationRepository(var permitted: Boolean = true) : LocationRepository {
    val fixes = MutableSharedFlow<LatLng>(extraBufferCapacity = 8)
    override fun hasPermission(): Boolean = permitted
    override fun locationUpdates(): Flow<LatLng> = fixes
}

@OptIn(ExperimentalCoroutinesApi::class)
class MapViewModelTest {

    @get:Rule
    val mainRule = MainDispatcherRule()

    @Test
    fun `addMarker persists and surfaces in state`() = runTest(mainRule.dispatcher) {
        val markers = FakeMarkerRepository()
        val vm = MapViewModel(markers, FakeLocationRepository())
        vm.addMarker("Camp", ActivityKindId.Camping, LatLng(69.0, 18.0))
        advanceUntilIdle()

        assertEquals(1, vm.state.value.markers.size)
        assertEquals("Camp", vm.state.value.markers[0].name)
        assertEquals(ActivityKindId.Camping, vm.state.value.markers[0].kind)
    }

    @Test
    fun `addMarker keeps the chosen colour and notes`() = runTest(mainRule.dispatcher) {
        val markers = FakeMarkerRepository()
        val vm = MapViewModel(markers, FakeLocationRepository())
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
        val vm = MapViewModel(markers, FakeLocationRepository())
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
        val vm = MapViewModel(markers, FakeLocationRepository())
        vm.addMarker("  ", ActivityKindId.Cabin, LatLng(69.0, 18.0))
        advanceUntilIdle()
        assertEquals(ActivityKindId.Cabin.label, vm.state.value.markers[0].name)
    }

    @Test
    fun `deleteMarker removes it`() = runTest(mainRule.dispatcher) {
        val markers = FakeMarkerRepository()
        markers.markers.value = listOf(Marker("m1", "A", ActivityKindId.Cabin, LatLng(69.0, 18.0)))
        val vm = MapViewModel(markers, FakeLocationRepository())
        advanceUntilIdle()
        assertEquals(1, vm.state.value.markers.size)

        vm.deleteMarker("m1")
        advanceUntilIdle()
        assertTrue(vm.state.value.markers.isEmpty())
    }

    @Test
    fun `enableLocation streams fixes into state`() = runTest(mainRule.dispatcher) {
        val location = FakeLocationRepository(permitted = true)
        val vm = MapViewModel(FakeMarkerRepository(), location)
        vm.enableLocation()
        runCurrent()
        location.fixes.emit(LatLng(69.65, 18.95))
        runCurrent()

        assertEquals(LatLng(69.65, 18.95), vm.state.value.userLocation)
    }

    @Test
    fun `enableLocation is a no-op without permission`() = runTest(mainRule.dispatcher) {
        val location = FakeLocationRepository(permitted = false)
        val vm = MapViewModel(FakeMarkerRepository(), location)
        vm.enableLocation()
        runCurrent()
        location.fixes.emit(LatLng(1.0, 2.0))
        runCurrent()
        assertEquals(null, vm.state.value.userLocation)
    }
}
