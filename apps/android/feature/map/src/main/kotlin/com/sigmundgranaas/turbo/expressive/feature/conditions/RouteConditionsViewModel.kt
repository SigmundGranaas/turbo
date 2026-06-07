package com.sigmundgranaas.turbo.expressive.feature.conditions

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

sealed interface RouteConditionsUiState {
    data object Idle : RouteConditionsUiState
    data object Loading : RouteConditionsUiState
    data class Content(val summary: RouteConditions) : RouteConditionsUiState
    data object Error : RouteConditionsUiState
}

/**
 * Fetches conditions at a handful of points along a route and rolls them into a
 * single [RouteConditions] for the route card. Memoised by the sampled endpoints
 * so re-composition (or a pan) doesn't re-fetch the same line.
 */
@HiltViewModel
class RouteConditionsViewModel @Inject constructor(
    private val repository: ConditionsRepository,
) : ViewModel() {
    private val _state = MutableStateFlow<RouteConditionsUiState>(RouteConditionsUiState.Idle)
    val state: StateFlow<RouteConditionsUiState> = _state.asStateFlow()

    private var loadedSig: String? = null

    fun load(geometry: List<LatLng>) {
        val points = sampleAlong(geometry, SAMPLE_COUNT)
        if (points.size < 2) {
            loadedSig = null
            _state.value = RouteConditionsUiState.Idle
            return
        }
        val sig = "${points.first()}|${points.last()}|${points.size}"
        if (sig == loadedSig && _state.value !is RouteConditionsUiState.Error) return
        loadedSig = sig
        _state.value = RouteConditionsUiState.Loading
        viewModelScope.launch {
            val ok = points
                .map { p -> async { repository.forPoint(p) } }
                .awaitAll()
                .mapNotNull { (it as? Outcome.Success)?.value }
            _state.value = if (ok.isEmpty()) {
                RouteConditionsUiState.Error
            } else {
                RouteConditionsUiState.Content(aggregateConditions(ok))
            }
        }
    }

    companion object {
        /** Sample points along the route (start, two interior, end). */
        const val SAMPLE_COUNT = 4
    }
}
