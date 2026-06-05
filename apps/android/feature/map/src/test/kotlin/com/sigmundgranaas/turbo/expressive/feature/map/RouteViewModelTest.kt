package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
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
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Result(plan))), FakePathRepository())
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
        )
        vm.planRoute(a, b)
        advanceUntilIdle()

        val done = vm.state.value as RouteUiState.Done
        assertEquals(plan, done.plan)
        assertEquals(plan.geometry, vm.state.value.polyline)
    }

    @Test
    fun `failure event becomes Error`() = runTest(mainRule.dispatcher) {
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Failure("no route"))), FakePathRepository())
        vm.planRoute(a, b)
        advanceUntilIdle()
        assertEquals("no route", (vm.state.value as RouteUiState.Error).message)
    }

    @Test
    fun `saveAsTrack persists the route as a Route GeoPath`() = runTest(mainRule.dispatcher) {
        val paths = FakePathRepository()
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Result(plan))), paths)
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
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Result(plan))), FakePathRepository())
        vm.planRoute(a, b)
        advanceUntilIdle()
        vm.clear()
        assertTrue(vm.state.value is RouteUiState.Idle)
    }

    @Test
    fun `selectPreset re-plans the same trip with the new style`() = runTest(mainRule.dispatcher) {
        val repo = FakeRouteRepository(listOf(RouteStreamEvent.Result(plan)))
        val vm = RouteViewModel(repo, FakePathRepository())
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
        val vm = RouteViewModel(FakeRouteRepository(listOf(RouteStreamEvent.Result(plan))), FakePathRepository())
        vm.planRoute(a, b)
        advanceUntilIdle()
        vm.follow()

        val following = vm.state.value as RouteUiState.Following
        assertEquals(plan, following.plan)
        assertEquals(plan.geometry, vm.state.value.polyline)
    }
}
