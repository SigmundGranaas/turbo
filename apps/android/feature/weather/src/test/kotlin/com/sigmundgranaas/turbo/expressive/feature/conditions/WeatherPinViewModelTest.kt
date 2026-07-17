package com.sigmundgranaas.turbo.expressive.feature.conditions

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.Conditions
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.MarineNow
import com.sigmundgranaas.turbo.expressive.domain.MarkerKind
import com.sigmundgranaas.turbo.expressive.domain.WeatherForecast
import com.sigmundgranaas.turbo.expressive.domain.WeatherNow
import com.sigmundgranaas.turbo.expressive.domain.WeatherSnapshot
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

/** Fresh MET fetch = 12 °C; a pin that refreshes should show it, a pin that doesn't must keep its cache. */
private class FakeConditions(private val outcome: Outcome<Conditions>) : ConditionsRepository {
    override suspend fun forPoint(point: LatLng): Outcome<Conditions> = outcome
    override suspend fun forecast(point: LatLng): Outcome<WeatherForecast> = Outcome.Failure(UnsupportedOperationException())
    override suspend fun marine(point: LatLng): Outcome<MarineNow?> = Outcome.Success(null)
}

private class RecordingMarkerRepository : MarkerRepository {
    val saved = MutableStateFlow<Marker?>(null)
    override fun observeAll(): Flow<List<Marker>> = MutableStateFlow(emptyList())
    override suspend fun upsert(marker: Marker) { saved.value = marker }
    override suspend fun delete(id: String) {}
}

@OptIn(ExperimentalCoroutinesApi::class)
class WeatherPinViewModelTest {

    @get:Rule
    val mainRule = MainDispatcherRule()

    private val fresh = Outcome.Success(
        Conditions(WeatherNow(12.0, 5.0, 180.0, 0.0, "clearsky_day"), null, MarineNow(0.4, 200.0, 9.0)),
    )

    /** A pin whose cache reads 5 °C, fetched at the epoch — always stale against the real clock. */
    private fun staleCachedPin() = Marker(
        id = "w-1",
        name = "Weather pin",
        kind = ActivityKindId.Viewpoint,
        position = LatLng(69.6, 18.9),
        markerKind = MarkerKind.WeatherPin,
        forecast = WeatherSnapshot(temperatureC = 5.0, symbolCode = "cloudy", windSpeedMs = 2.0, windFromDeg = 90.0, precipitationMm = 0.0),
        forecastFetchedAtEpochMs = 0L,
    )

    @Test
    fun `stale pin online refreshes to the live forecast and caches it on the node`() = runTest(mainRule.dispatcher) {
        val repo = RecordingMarkerRepository()
        val vm = WeatherPinViewModel(FakeConditions(fresh), repo)

        vm.open(staleCachedPin(), online = true)
        advanceUntilIdle()

        // User-visible: the pin now shows the fresh 12°, and the fresh forecast was written back.
        assertEquals(12.0, vm.state.value!!.temperatureC!!, 1e-9)
        assertEquals(12.0, repo.saved.value!!.forecast!!.temperatureC!!, 1e-9)
    }

    @Test
    fun `offline pin keeps its cached forecast and never throws`() = runTest(mainRule.dispatcher) {
        val vm = WeatherPinViewModel(FakeConditions(fresh), RecordingMarkerRepository())

        vm.open(staleCachedPin(), online = false)
        advanceUntilIdle()

        // Offline: the cached 5° stands (no blank, no crash), even though the fetch would return 12°.
        assertEquals(5.0, vm.state.value!!.temperatureC!!, 1e-9)
    }

    @Test
    fun `a failed refresh keeps the cached forecast`() = runTest(mainRule.dispatcher) {
        val vm = WeatherPinViewModel(FakeConditions(Outcome.Failure(RuntimeException("MET down"))), RecordingMarkerRepository())

        vm.open(staleCachedPin(), online = true)
        advanceUntilIdle()

        assertEquals(5.0, vm.state.value!!.temperatureC!!, 1e-9)
    }

    @Test
    fun `expanded state exposes the cached wind, precipitation and marine detail`() = runTest(mainRule.dispatcher) {
        val vm = WeatherPinViewModel(FakeConditions(fresh), RecordingMarkerRepository())
        vm.open(staleCachedPin(), online = false)
        advanceUntilIdle()
        val e = vm.state.value!!.expanded
        assertEquals(2.0, e.windSpeedMs!!, 1e-9)
        assertEquals(90.0, e.windFromDeg!!, 1e-9)
        assertEquals(0.0, e.precipitationMm!!, 1e-9)
    }
}
