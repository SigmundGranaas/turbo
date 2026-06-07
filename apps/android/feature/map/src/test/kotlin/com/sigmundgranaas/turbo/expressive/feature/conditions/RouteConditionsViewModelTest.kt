package com.sigmundgranaas.turbo.expressive.feature.conditions

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
import com.sigmundgranaas.turbo.expressive.domain.AvalancheNow
import com.sigmundgranaas.turbo.expressive.domain.Conditions
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.WeatherForecast
import com.sigmundgranaas.turbo.expressive.domain.WeatherNow
import com.sigmundgranaas.turbo.expressive.feature.map.MainDispatcherRule
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class RouteConditionsViewModelTest {

    @get:Rule
    val mainDispatcher = MainDispatcherRule()

    private class FakeConditionsRepository(
        private val byTemp: Double,
        private val byDanger: Int,
        var failAll: Boolean = false,
        var calls: Int = 0,
    ) : ConditionsRepository {
        override suspend fun forPoint(point: LatLng): Outcome<Conditions> {
            calls++
            if (failAll) return Outcome.Failure(RuntimeException("offline"))
            // Vary by latitude so the aggregate sees a real range.
            return Outcome.Success(
                Conditions(
                    weather = WeatherNow(byTemp + point.lat, null, null, null, null),
                    avalanche = AvalancheNow(dangerLevel = byDanger, mainText = "", region = "Test"),
                ),
            )
        }

        override suspend fun forecast(point: LatLng): Outcome<WeatherForecast> =
            Outcome.Failure(RuntimeException("not used"))
    }

    private fun line() = (0 until 10).map { LatLng(60.0 + it, 8.0) }

    @Test
    fun `load samples the route and rolls up a content summary`() = runTest {
        val repo = FakeConditionsRepository(byTemp = 0.0, byDanger = 3)
        val vm = RouteConditionsViewModel(repo)

        vm.load(line())
        advanceUntilIdle()

        val state = vm.state.value
        assertTrue(state is RouteConditionsUiState.Content)
        val summary = (state as RouteConditionsUiState.Content).summary
        assertEquals(RouteConditionsViewModel.SAMPLE_COUNT, repo.calls)
        assertEquals(3, summary.worstDanger)
        // first sample lat 60 → 60.0, last sample lat 69 → 69.0
        assertEquals(60.0, summary.tempMinC!!, 0.001)
        assertEquals(69.0, summary.tempMaxC!!, 0.001)
    }

    @Test
    fun `a too-short line stays idle and does not fetch`() = runTest {
        val repo = FakeConditionsRepository(byTemp = 0.0, byDanger = 1)
        val vm = RouteConditionsViewModel(repo)

        vm.load(listOf(LatLng(60.0, 8.0)))
        advanceUntilIdle()

        assertTrue(vm.state.value is RouteConditionsUiState.Idle)
        assertEquals(0, repo.calls)
    }

    @Test
    fun `all-failures surface as error`() = runTest {
        val repo = FakeConditionsRepository(byTemp = 0.0, byDanger = 1, failAll = true)
        val vm = RouteConditionsViewModel(repo)

        vm.load(line())
        advanceUntilIdle()

        assertTrue(vm.state.value is RouteConditionsUiState.Error)
    }

    @Test
    fun `the same line is memoised and not re-fetched`() = runTest {
        val repo = FakeConditionsRepository(byTemp = 0.0, byDanger = 2)
        val vm = RouteConditionsViewModel(repo)

        vm.load(line())
        advanceUntilIdle()
        val after = repo.calls
        vm.load(line())
        advanceUntilIdle()

        assertEquals(after, repo.calls)
    }
}
