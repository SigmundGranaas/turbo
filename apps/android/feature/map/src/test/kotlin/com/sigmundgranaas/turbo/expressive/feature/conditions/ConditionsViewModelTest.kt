package com.sigmundgranaas.turbo.expressive.feature.conditions

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
import com.sigmundgranaas.turbo.expressive.domain.Conditions
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.WeatherNow
import com.sigmundgranaas.turbo.expressive.feature.map.MainDispatcherRule
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

private class FakeConditionsRepository(var outcome: Outcome<Conditions>) : ConditionsRepository {
    var calls = 0
    override suspend fun forPoint(point: LatLng): Outcome<Conditions> {
        calls++
        return outcome
    }
}

@OptIn(ExperimentalCoroutinesApi::class)
class ConditionsViewModelTest {

    @get:Rule
    val mainRule = MainDispatcherRule()

    private val point = LatLng(69.6, 18.9)
    private val conditions = Conditions(WeatherNow(-2.0, 4.0, 315.0, 0.2, "cloudy"), null)

    @Test
    fun `load success exposes Content`() = runTest(mainRule.dispatcher) {
        val vm = ConditionsViewModel(FakeConditionsRepository(Outcome.Success(conditions)))
        vm.load(point)
        advanceUntilIdle()
        val content = vm.state.value as ConditionsUiState.Content
        assertEquals(-2.0, content.conditions.weather!!.temperatureC!!, 1e-9)
    }

    @Test
    fun `load failure exposes Error`() = runTest(mainRule.dispatcher) {
        val vm = ConditionsViewModel(FakeConditionsRepository(Outcome.Failure(RuntimeException("offline"))))
        vm.load(point)
        advanceUntilIdle()
        assertTrue(vm.state.value is ConditionsUiState.Error)
    }

    @Test
    fun `loading the same point twice fetches once`() = runTest(mainRule.dispatcher) {
        val repo = FakeConditionsRepository(Outcome.Success(conditions))
        val vm = ConditionsViewModel(repo)
        vm.load(point)
        advanceUntilIdle()
        vm.load(point)
        advanceUntilIdle()
        assertEquals(1, repo.calls)
    }
}
