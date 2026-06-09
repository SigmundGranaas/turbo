package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
import com.sigmundgranaas.turbo.expressive.core.data.TideRepository
import com.sigmundgranaas.turbo.expressive.domain.Conditions
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.MarineNow
import com.sigmundgranaas.turbo.expressive.domain.TideForecast
import com.sigmundgranaas.turbo.expressive.domain.WeatherForecast
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

sealed interface ConditionsUiState {
    data object Loading : ConditionsUiState
    data class Content(val conditions: Conditions) : ConditionsUiState
    data object Error : ConditionsUiState
}

sealed interface ForecastUiState {
    data object Loading : ForecastUiState
    data class Content(val forecast: WeatherForecast) : ForecastUiState
    data object Error : ForecastUiState
}

/** Ocean tab: marine (waves/sea-temp/current) + tide extrema. [Empty] off the coast. */
sealed interface OceanUiState {
    data object Loading : OceanUiState
    data class Content(val marine: MarineNow?, val tides: TideForecast?) : OceanUiState
    data object Empty : OceanUiState
}

/** Loads live conditions (MET weather + Varsom danger) for a point on demand. */
@HiltViewModel
class ConditionsViewModel @Inject constructor(
    private val repository: ConditionsRepository,
    private val tides: TideRepository,
) : ViewModel() {
    private val _state = MutableStateFlow<ConditionsUiState>(ConditionsUiState.Loading)
    val state: StateFlow<ConditionsUiState> = _state.asStateFlow()

    private val _forecast = MutableStateFlow<ForecastUiState>(ForecastUiState.Loading)
    val forecast: StateFlow<ForecastUiState> = _forecast.asStateFlow()

    private val _ocean = MutableStateFlow<OceanUiState>(OceanUiState.Loading)
    val ocean: StateFlow<OceanUiState> = _ocean.asStateFlow()

    private var loadedFor: LatLng? = null
    private var forecastFor: LatLng? = null
    private var oceanFor: LatLng? = null

    fun load(point: LatLng) {
        if (loadedFor == point) return
        loadedFor = point
        _state.value = ConditionsUiState.Loading
        viewModelScope.launch {
            _state.value = when (val outcome = repository.forPoint(point)) {
                is Outcome.Success -> ConditionsUiState.Content(outcome.value)
                is Outcome.Failure -> ConditionsUiState.Error
            }
        }
    }

    /** Loads the full hourly+daily forecast for the detail sheet (memoised per point). */
    fun loadForecast(point: LatLng) {
        if (forecastFor == point && _forecast.value !is ForecastUiState.Error) return
        forecastFor = point
        _forecast.value = ForecastUiState.Loading
        viewModelScope.launch {
            _forecast.value = when (val outcome = repository.forecast(point)) {
                is Outcome.Success -> ForecastUiState.Content(outcome.value)
                is Outcome.Failure -> ForecastUiState.Error
            }
        }
    }

    /** Loads marine + tides for the ocean section (memoised; [Empty] off the coast). */
    fun loadOcean(point: LatLng) {
        if (oceanFor == point && _ocean.value !is OceanUiState.Empty) return
        oceanFor = point
        _ocean.value = OceanUiState.Loading
        viewModelScope.launch {
            val (marine, tide) = coroutineScope {
                val m = async { (repository.marine(point) as? Outcome.Success)?.value }
                val t = async { (tides.forPoint(point) as? Outcome.Success)?.value?.takeIf { it.hasData } }
                m.await() to t.await()
            }
            _ocean.value = if (marine == null && tide == null) {
                OceanUiState.Empty
            } else {
                OceanUiState.Content(marine, tide)
            }
        }
    }
}
