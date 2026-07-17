package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.WeatherPinFetch
import com.sigmundgranaas.turbo.expressive.domain.WeatherPinUiState
import com.sigmundgranaas.turbo.expressive.domain.WeatherSnapshot
import com.sigmundgranaas.turbo.expressive.domain.weatherPinFetchDecision
import com.sigmundgranaas.turbo.expressive.domain.weatherPinUiState
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * Drives a weather pin's detail: paints instantly from the node's cached forecast, then
 * — only when **online** and the cache is **stale** — re-fetches MET and writes the fresh
 * forecast back onto the marker. Offline (or on a fetch error) it keeps the cache and never
 * throws. The staleness/online decision and the render mapping are the pure seams in
 * `core:model` (`weatherPinFetchDecision` / `weatherPinUiState`); this VM only orchestrates.
 */
@HiltViewModel
class WeatherPinViewModel @Inject constructor(
    private val conditions: ConditionsRepository,
    private val markers: MarkerRepository,
) : ViewModel() {

    private val _state = MutableStateFlow<WeatherPinUiState?>(null)
    /** Render state for the open pin — null only while a freshly dropped pin has no cache yet. */
    val state: StateFlow<WeatherPinUiState?> = _state.asStateFlow()

    private val now: Long get() = System.currentTimeMillis()

    /**
     * Open a weather pin: render its cache immediately, then refresh if [online] and stale.
     * [online] is passed in (from the screen's connectivity) so the decision stays a driven,
     * device-free seam.
     */
    fun open(marker: Marker, online: Boolean) {
        _state.value = weatherPinUiState(marker, now)
        val cacheAge = marker.forecastFetchedAtEpochMs?.let { (now - it).coerceAtLeast(0L) }
        if (weatherPinFetchDecision(cacheAge, online) == WeatherPinFetch.UseCache) return
        viewModelScope.launch {
            when (val outcome = conditions.forPoint(marker.position)) {
                is Outcome.Success -> {
                    val refreshed = marker.copy(
                        forecast = WeatherSnapshot.from(outcome.value),
                        forecastFetchedAtEpochMs = now,
                    )
                    _state.value = weatherPinUiState(refreshed, now)
                    markers.upsert(refreshed) // persist the fresh cache onto the node
                }
                // Offline / MET down: the cache we already emitted stands. Never blank, never throw.
                is Outcome.Failure -> Unit
            }
        }
    }
}
