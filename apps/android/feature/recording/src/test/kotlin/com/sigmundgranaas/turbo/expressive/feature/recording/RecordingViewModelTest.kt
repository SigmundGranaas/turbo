package com.sigmundgranaas.turbo.expressive.feature.recording

import com.sigmundgranaas.turbo.expressive.core.data.LocationRepository
import com.sigmundgranaas.turbo.expressive.core.data.LocationSample
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.data.RecordingController
import com.sigmundgranaas.turbo.expressive.core.data.RecordingDraft
import com.sigmundgranaas.turbo.expressive.core.data.RecordingDraftStore
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

private class FakeRecordingLauncher : RecordingLauncher {
    var starts = 0
    var stops = 0
    override fun start() { starts++ }
    override fun stop() { stops++ }
}

private class FakeLocationRepository(var permitted: Boolean = true) : LocationRepository {
    val feed = MutableSharedFlow<LocationSample>(extraBufferCapacity = 8)
    override fun hasPermission(): Boolean = permitted
    override fun samples(): Flow<LocationSample> = feed
    suspend fun emit(lat: Double, lng: Double, alt: Double? = null) = feed.emit(LocationSample(LatLng(lat, lng), alt))
}

private class FakePathRepository : PathRepository {
    val saved = mutableListOf<SavedPath>()
    override fun observeAll(): Flow<List<SavedPath>> = MutableStateFlow(emptyList())
    override suspend fun byId(id: String): SavedPath? = saved.firstOrNull { it.id == id }
    override suspend fun save(path: SavedPath) { saved += path }
    override suspend fun delete(id: String) { saved.removeAll { it.id == id } }
    override suspend fun remoteId(id: String): String? = null
}

private class NoopDraftStore : RecordingDraftStore {
    override suspend fun load(): RecordingDraft? = null
    override suspend fun save(points: List<LatLng>, elevations: List<Double?>, elapsedSec: Int) = Unit
    override suspend fun clear() = Unit
}

@OptIn(ExperimentalCoroutinesApi::class)
class RecordingViewModelTest {

    @get:Rule
    val mainRule = MainDispatcherRule()

    private fun vmWith(
        scope: CoroutineScope,
        location: FakeLocationRepository = FakeLocationRepository(),
        launcher: FakeRecordingLauncher = FakeRecordingLauncher(),
        paths: FakePathRepository = FakePathRepository(),
    ): Triple<RecordingViewModel, RecordingController, FakeRecordingLauncher> {
        val controller = RecordingController(location, NoopDraftStore(), scope)
        val vm = RecordingViewModel(launcher, controller, location, paths)
        return Triple(vm, controller, launcher)
    }

    @Test
    fun `granting permission starts the service`() = runTest(mainRule.dispatcher) {
        val (vm, _, launcher) = vmWith(backgroundScope)
        vm.onPermissionResult(true)
        assertEquals(1, launcher.starts)
    }

    @Test
    fun `start without permission does not launch`() = runTest(mainRule.dispatcher) {
        val (vm, _, launcher) = vmWith(backgroundScope, location = FakeLocationRepository(permitted = false))
        vm.start()
        assertEquals(0, launcher.starts)
    }

    @Test
    fun `stop delegates to the launcher`() = runTest(mainRule.dispatcher) {
        val (vm, _, launcher) = vmWith(backgroundScope)
        vm.stop()
        assertEquals(1, launcher.stops)
    }

    @Test
    fun `state mirrors the controller session`() = runTest(mainRule.dispatcher) {
        val location = FakeLocationRepository()
        val (vm, controller, _) = vmWith(backgroundScope, location = location)
        // state is WhileSubscribed — activate an observer (the screen does this in prod).
        backgroundScope.launch { vm.state.collect { } }
        controller.start()
        runCurrent()
        location.emit(69.0, 18.0); runCurrent()
        location.emit(69.001, 18.0); runCurrent()
        advanceUntilIdle()

        assertTrue(vm.state.value.recording)
        assertEquals(2, vm.state.value.points.size)
    }

    @Test
    fun `save persists the recorded track and resets`() = runTest(mainRule.dispatcher) {
        val location = FakeLocationRepository()
        val paths = FakePathRepository()
        val (vm, controller, _) = vmWith(backgroundScope, location = location, paths = paths)
        controller.start()
        runCurrent()
        location.emit(69.0, 18.0, alt = 10.0); runCurrent()
        location.emit(69.001, 18.0, alt = 30.0); runCurrent()

        var savedCalled = false
        vm.save("Hike") { savedCalled = true }
        advanceUntilIdle()

        assertTrue(savedCalled)
        assertEquals(1, paths.saved.size)
        assertEquals("Hike", paths.saved[0].name)
        assertEquals(GeoPathSource.Recording, paths.saved[0].path.source)
        assertEquals(2, paths.saved[0].path.points.size)
        // Altitude was captured → elevations + ascent persisted on the GeoPath.
        assertEquals(listOf(10.0, 30.0), paths.saved[0].path.elevations)
        assertEquals(20.0, paths.saved[0].path.ascentM!!, 1e-6)
        // Session cleared after save.
        assertTrue(controller.session.value.points.isEmpty())
    }

    @Test
    fun `discard resets without persisting`() = runTest(mainRule.dispatcher) {
        val location = FakeLocationRepository()
        val paths = FakePathRepository()
        val (vm, controller, _) = vmWith(backgroundScope, location = location, paths = paths)
        controller.start()
        runCurrent()
        location.emit(69.0, 18.0); runCurrent()

        var done = false
        vm.discard { done = true }
        assertTrue(done)
        assertEquals(0, paths.saved.size)
        assertTrue(controller.session.value.points.isEmpty())
    }
}
