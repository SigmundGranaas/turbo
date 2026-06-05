package com.sigmundgranaas.turbo.expressive.feature.search

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.SearchRepository
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SearchHit
import kotlinx.coroutines.ExperimentalCoroutinesApi
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

@OptIn(ExperimentalCoroutinesApi::class)
class SearchViewModelTest {

    @get:Rule
    val mainRule = MainDispatcherRule()

    private val hit = SearchHit("Lyngen", "Troms", LatLng(69.6, 20.0))

    @Test
    fun `query is debounced then mapped to results with coordinates`() = runTest(mainRule.dispatcher) {
        val repo = FakeSearchRepository(Outcome.Success(listOf(hit)))
        val vm = SearchViewModel(repo)

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
        val vm = SearchViewModel(repo)

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
        val vm = SearchViewModel(repo)
        vm.setQuery("Lyn"); advanceTimeBy(300); runCurrent()
        assertEquals(1, vm.state.value.results.size)

        vm.setQuery("")
        assertTrue(vm.state.value.results.isEmpty())
    }

    @Test
    fun `failure yields empty results and clears loading`() = runTest(mainRule.dispatcher) {
        val repo = FakeSearchRepository(Outcome.Failure(RuntimeException("offline")))
        val vm = SearchViewModel(repo)
        vm.setQuery("zzz"); advanceTimeBy(300); runCurrent()

        assertTrue(vm.state.value.results.isEmpty())
        assertTrue(!vm.state.value.loading)
    }
}
