package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.core.map.OfflineTileManager
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.OfflineRegionInfo
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.domain.RouteStreamEvent
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

private class FakeRouteRepository(private val events: List<RouteStreamEvent>) : RouteRepository {
    var calls = 0
    var lastPreset: RoutePreset? = null
    override fun planStream(points: List<LatLng>, preset: RoutePreset, profile: String): Flow<RouteStreamEvent> {
        calls++
        lastPreset = preset
        return flowOf(*events.toTypedArray())
    }
}

private class FakeOfflineTileManager : OfflineTileManager {
    var downloads = 0
    var lastBounds: GeoBounds? = null
    override val regions = MutableStateFlow<List<OfflineRegionInfo>>(emptyList())
    override fun refresh() = Unit
    override fun download(name: String, base: BaseLayer, bounds: GeoBounds, minZoom: Double, maxZoom: Double) {
        downloads++; lastBounds = bounds
    }
    override fun delete(id: Long) = Unit
}

private class FakePathRepository : PathRepository {
    val saved = mutableListOf<SavedPath>()
    val all = MutableStateFlow<List<SavedPath>>(emptyList())
    override fun observeAll(): Flow<List<SavedPath>> = all
    override suspend fun byId(id: String): SavedPath? = saved.firstOrNull { it.id == id }
    override suspend fun save(path: SavedPath) { saved += path }
    override suspend fun delete(id: String) { saved.removeAll { it.id == id } }
}

@OptIn(ExperimentalCoroutinesApi::class)
class RouteViewModelTest {

    @get:Rule
    val mainRule = MainDispatcherRule()

    private val a = LatLng(69.0, 18.0)
    private val b = LatLng(69.01, 18.01)
    private val plan = RoutePlan(
        distanceM = 1500.0, durationS = 1200.0, ascentM = 80.0, onTrailPct = 90.0,
        surfaces = mapOf("trail" to 1500.0),
        geometry = listOf(a, LatLng(69.005, 18.005), b),
    )

    @Test
    fun `planRoute seeds a straight line before solving`() = runTest(mainRule.dispatcher) {
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Result(plan))), FakePathRepository(), FakeOfflineTileManager())
        vm.planRoute(a, b)
        // Synchronous seed, before the stream is collected.
        val s = vm.state.value as RouteUiState.Solving
        assertEquals(listOf(a, b), s.progress)
    }

    @Test
    fun `streamed result becomes Done with the plan geometry`() = runTest(mainRule.dispatcher) {
        val vm = RouteViewModel(
            FakeRouteRepository(
                listOf(
                    RouteStreamEvent.Progress(listOf(a, LatLng(69.004, 18.004))),
                    RouteStreamEvent.Result(plan),
                ),
            ),
            FakePathRepository(),
            FakeOfflineTileManager(),
        )
        vm.planRoute(a, b)
        advanceUntilIdle()

        val done = vm.state.value as RouteUiState.Done
        assertEquals(plan, done.plan)
        assertEquals(plan.geometry, vm.state.value.polyline)
    }

    @Test
    fun `failure event becomes Error`() = runTest(mainRule.dispatcher) {
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Failure("no route"))), FakePathRepository(), FakeOfflineTileManager())
        vm.planRoute(a, b)
        advanceUntilIdle()
        assertEquals("no route", (vm.state.value as RouteUiState.Error).message)
    }

    @Test
    fun `saveAsTrack persists the route as a Route GeoPath`() = runTest(mainRule.dispatcher) {
        val paths = FakePathRepository()
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Result(plan))), paths, FakeOfflineTileManager())
        vm.planRoute(a, b)
        advanceUntilIdle()
        vm.saveAsTrack("Trip")
        advanceUntilIdle()

        assertEquals(1, paths.saved.size)
        assertEquals("Trip", paths.saved[0].name)
        assertEquals(GeoPathSource.Route, paths.saved[0].path.source)
        assertEquals(plan.geometry, paths.saved[0].path.points)
        assertEquals(1500.0, paths.saved[0].path.distanceM, 1e-6)
    }

    @Test
    fun `clear resets to Idle`() = runTest(mainRule.dispatcher) {
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Result(plan))), FakePathRepository(), FakeOfflineTileManager())
        vm.planRoute(a, b)
        advanceUntilIdle()
        vm.clear()
        assertTrue(vm.state.value is RouteUiState.Idle)
    }

    @Test
    fun `selectPreset re-plans the same trip with the new style`() = runTest(mainRule.dispatcher) {
        val repo = FakeRouteRepository(listOf(RouteStreamEvent.Result(plan)))
        val vm = RouteViewModel(repo, FakePathRepository(), FakeOfflineTileManager())
        vm.planRoute(a, b)
        advanceUntilIdle()
        assertEquals(1, repo.calls)

        vm.selectPreset(RoutePreset.AvoidRoads)
        advanceUntilIdle()

        assertEquals(2, repo.calls)
        assertEquals(RoutePreset.AvoidRoads, repo.lastPreset)
        assertEquals(RoutePreset.AvoidRoads, vm.preset.value)
        assertTrue(vm.state.value is RouteUiState.Done)
    }

    @Test
    fun `follow moves a solved route into Following with the same geometry`() = runTest(mainRule.dispatcher) {
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Result(plan))), FakePathRepository(), FakeOfflineTileManager())
        vm.planRoute(a, b)
        advanceUntilIdle()
        vm.follow()

        val following = vm.state.value as RouteUiState.Following
        assertEquals(plan, following.plan)
        assertEquals(plan.geometry, vm.state.value.polyline)
    }

    @Test
    fun `reroute re-solves from the new origin and stays in Following`() = runTest(mainRule.dispatcher) {
        val repo = FakeRouteRepository(listOf(RouteStreamEvent.Result(plan)))
        val vm = RouteViewModel(repo, FakePathRepository(), FakeOfflineTileManager())
        vm.planRoute(a, b)
        advanceUntilIdle()
        vm.follow()

        vm.reroute(LatLng(69.5, 18.5))
        advanceUntilIdle()

        assertEquals(2, repo.calls)
        assertTrue(vm.state.value is RouteUiState.Following)
    }

    @Test
    fun `reroute is ignored when not following`() = runTest(mainRule.dispatcher) {
        val repo = FakeRouteRepository(listOf(RouteStreamEvent.Result(plan)))
        val vm = RouteViewModel(repo, FakePathRepository(), FakeOfflineTileManager())
        vm.planRoute(a, b)
        advanceUntilIdle() // Done, not Following
        vm.reroute(LatLng(69.5, 18.5))
        advanceUntilIdle()
        assertEquals(1, repo.calls)
    }

    @Test
    fun `addStop inserts a least-detour waypoint and re-solves`() = runTest(mainRule.dispatcher) {
        val repo = FakeRouteRepository(listOf(RouteStreamEvent.Result(plan)))
        val vm = RouteViewModel(repo, FakePathRepository(), FakeOfflineTileManager())
        vm.planRoute(a, b)
        advanceUntilIdle()
        assertEquals(2, vm.waypoints.value.size)

        val stop = LatLng(69.005, 18.005)
        vm.addStop(stop)
        advanceUntilIdle()

        assertEquals(listOf(a, stop, b), vm.waypoints.value)
        assertEquals(2, repo.calls)
    }

    @Test
    fun `removeWaypoint drops a stop and undo restores it`() = runTest(mainRule.dispatcher) {
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Result(plan))), FakePathRepository(), FakeOfflineTileManager())
        vm.planRoute(a, b); advanceUntilIdle()
        val stop = LatLng(69.005, 18.005)
        vm.addStop(stop); advanceUntilIdle()
        assertEquals(3, vm.waypoints.value.size)

        vm.removeWaypoint(1); advanceUntilIdle()
        assertEquals(listOf(a, b), vm.waypoints.value)

        vm.undo(); advanceUntilIdle()
        assertEquals(listOf(a, stop, b), vm.waypoints.value)
    }

    @Test
    fun `addStop is a no-op before a route exists`() = runTest(mainRule.dispatcher) {
        val repo = FakeRouteRepository(listOf(RouteStreamEvent.Result(plan)))
        val vm = RouteViewModel(repo, FakePathRepository(), FakeOfflineTileManager())
        vm.addStop(LatLng(69.005, 18.005))
        advanceUntilIdle()
        assertEquals(0, repo.calls)
        assertTrue(vm.waypoints.value.isEmpty())
    }

    @Test
    fun `downloadAlongRoute queues a corridor download for a solved route`() = runTest(mainRule.dispatcher) {
        val offline = FakeOfflineTileManager()
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Result(plan))), FakePathRepository(), offline)
        vm.planRoute(a, b); advanceUntilIdle()

        vm.downloadAlongRoute(BaseLayer.Norgeskart)
        assertEquals(1, offline.downloads)
        val box = offline.lastBounds!!
        assertTrue(box.south <= plan.geometry.minOf { it.lat } && box.north >= plan.geometry.maxOf { it.lat })
    }

    @Test
    fun `downloadAlongRoute is a no-op without a solved route`() = runTest(mainRule.dispatcher) {
        val offline = FakeOfflineTileManager()
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Result(plan))), FakePathRepository(), offline)
        vm.downloadAlongRoute(BaseLayer.Norgeskart)
        assertEquals(0, offline.downloads)
    }

    @Test
    fun `insertLeastDetour places a point on the cheapest segment`() {
        val w0 = LatLng(0.0, 0.0)
        val w1 = LatLng(0.0, 1.0)
        val w2 = LatLng(0.0, 2.0)
        // A point near the second segment should land between w1 and w2 (index 2).
        val near = LatLng(0.001, 1.5)
        val result = Waypoints.insertLeastDetour(listOf(w0, w1, w2), near)
        assertEquals(near, result[2])
        assertEquals(4, result.size)
    }
}
