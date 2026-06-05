package com.sigmundgranaas.turbo.expressive.feature.search

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.SearchRepository
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.SearchHit
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

private class FakeSearchRepository(
    var outcome: Outcome<List<SearchHit>>,
) : SearchRepository {
    var calls = 0
    var lastQuery: String? = null
    override suspend fun search(query: String): Outcome<List<SearchHit>> {
        calls++
        lastQuery = query
        return outcome
    }
}

private class FakeMarkerRepository(initial: List<Marker> = emptyList()) : MarkerRepository {
    val markers = MutableStateFlow(initial)
    override fun observeAll(): Flow<List<Marker>> = markers
    override suspend fun upsert(marker: Marker) { markers.value = markers.value.filterNot { it.id == marker.id } + marker }
    override suspend fun delete(id: String) { markers.value = markers.value.filterNot { it.id == id } }
}

@OptIn(ExperimentalCoroutinesApi::class)
class SearchViewModelTest {

    @get:Rule
    val mainRule = MainDispatcherRule()

    private val hit = SearchHit("Lyngen", "Troms", LatLng(69.6, 20.0))

    @Test
    fun `query is debounced then mapped to results with coordinates`() = runTest(mainRule.dispatcher) {
        val repo = FakeSearchRepository(Outcome.Success(listOf(hit)))
        val vm = SearchViewModel(repo, FakeMarkerRepository())

        vm.setQuery("Lyn")
        assertEquals("Lyn", vm.state.value.query)
        assertTrue("no results before debounce", vm.state.value.results.isEmpty())

        advanceTimeBy(300)
        runCurrent()

        assertEquals(1, vm.state.value.results.size)
        assertEquals("Lyngen", vm.state.value.results[0].name)
        assertEquals(69.6, vm.state.value.results[0].lat!!, 1e-9)
        assertTrue(!vm.state.value.loading)
    }

    @Test
    fun `rapid keystrokes only fire one search for the latest query`() = runTest(mainRule.dispatcher) {
        val repo = FakeSearchRepository(Outcome.Success(listOf(hit)))
        val vm = SearchViewModel(repo, FakeMarkerRepository())

        vm.setQuery("L")
        vm.setQuery("Ly")
        vm.setQuery("Lyn")
        advanceTimeBy(300)
        runCurrent()

        assertEquals(1, repo.calls)
        assertEquals("Lyn", repo.lastQuery)
    }

    @Test
    fun `blank query clears results without searching`() = runTest(mainRule.dispatcher) {
        val repo = FakeSearchRepository(Outcome.Success(listOf(hit)))
        val vm = SearchViewModel(repo, FakeMarkerRepository())
        vm.setQuery("Lyn"); advanceTimeBy(300); runCurrent()
        assertEquals(1, vm.state.value.results.size)

        vm.setQuery("")
        assertTrue(vm.state.value.results.isEmpty())
    }

    @Test
    fun `failure yields empty results and clears loading`() = runTest(mainRule.dispatcher) {
        val repo = FakeSearchRepository(Outcome.Failure(RuntimeException("offline")))
        val vm = SearchViewModel(repo, FakeMarkerRepository())
        vm.setQuery("zzz"); advanceTimeBy(300); runCurrent()

        assertTrue(vm.state.value.results.isEmpty())
        assertTrue(!vm.state.value.loading)
    }

    @Test
    fun `local markers matching the query appear instantly, before the network`() = runTest(mainRule.dispatcher) {
        val markers = FakeMarkerRepository(
            listOf(Marker("m1", "Camp Lyngen", ActivityKindId.Camping, LatLng(69.6, 20.0))),
        )
        val vm = SearchViewModel(FakeSearchRepository(Outcome.Success(emptyList())), markers)
        runCurrent() // let the marker flow collect

        vm.setQuery("lyng")
        runCurrent() // no debounce elapsed yet

        val marker = vm.state.value.results.single { it.type == SearchResultType.Marker }
        assertEquals("Camp Lyngen", marker.name)
    }

    @Test
    fun `a coordinate query yields a go-to-coordinate result`() = runTest(mainRule.dispatcher) {
        val vm = SearchViewModel(FakeSearchRepository(Outcome.Success(emptyList())), FakeMarkerRepository())
        vm.setQuery("69.65, 18.95")
        runCurrent()

        val coord = vm.state.value.results.single { it.type == SearchResultType.Coordinate }
        assertEquals(69.65, coord.lat!!, 1e-9)
        assertEquals(18.95, coord.lng!!, 1e-9)
    }

    @Test
    fun `the Places filter hides local markers but keeps places and coordinates`() = runTest(mainRule.dispatcher) {
        val markers = FakeMarkerRepository(
            listOf(Marker("m1", "Lyngen hut", ActivityKindId.Cabin, LatLng(69.6, 20.0))),
        )
        val vm = SearchViewModel(FakeSearchRepository(Outcome.Success(listOf(hit))), markers)
        runCurrent()

        vm.setQuery("Lyngen"); advanceTimeBy(300); runCurrent()
        assertTrue(vm.state.value.results.any { it.type == SearchResultType.Marker })

        vm.setFilter(2) // Places
        assertTrue(vm.state.value.results.none { it.type == SearchResultType.Marker })
        assertTrue(vm.state.value.results.any { it.type == SearchResultType.Place })
    }

    @Test
    fun `parseCoordinate accepts comma and space separators and rejects junk`() {
        assertEquals(LatLng(60.0, 10.5), parseCoordinate("60, 10.5"))
        assertEquals(LatLng(-60.0, 10.5), parseCoordinate("-60 10.5"))
        assertEquals(null, parseCoordinate("Lillehammer"))
        assertEquals(null, parseCoordinate("200, 10")) // out of range
    }
}
