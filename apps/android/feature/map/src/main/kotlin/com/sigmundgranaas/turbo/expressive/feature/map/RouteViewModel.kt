package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import com.sigmundgranaas.turbo.expressive.domain.RoutePreset
import com.sigmundgranaas.turbo.expressive.domain.RouteStreamEvent
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

sealed interface RouteUiState {
    data object Idle : RouteUiState
    data class Solving(val progress: List<LatLng>) : RouteUiState
    data class Done(val plan: RoutePlan) : RouteUiState
    data class Error(val message: String) : RouteUiState

    /** The polyline to draw for this state (empty when nothing to show). */
    val polyline: List<LatLng>
        get() = when (this) {
            is Solving -> progress
            is Done -> plan.geometry
            else -> emptyList()
        }
}

/** Drives a route solve against the Turbo pathfinder, streaming live progress. */
@HiltViewModel
class RouteViewModel @Inject constructor(
    private val routes: RouteRepository,
    private val paths: PathRepository,
) : ViewModel() {
    private val _state = MutableStateFlow<RouteUiState>(RouteUiState.Idle)
    val state: StateFlow<RouteUiState> = _state.asStateFlow()

    private var job: Job? = null

    fun planRoute(from: LatLng, to: LatLng, preset: RoutePreset = RoutePreset.Balanced) {
        job?.cancel()
        // Seed with the straight line so the user sees intent immediately.
        _state.value = RouteUiState.Solving(listOf(from, to))
        job = viewModelScope.launch {
            try {
                routes.planStream(listOf(from, to), preset).collect { event ->
                    _state.value = when (event) {
                        is RouteStreamEvent.Progress -> RouteUiState.Solving(event.coordinates)
                        is RouteStreamEvent.Result -> RouteUiState.Done(event.plan)
                        is RouteStreamEvent.Failure -> RouteUiState.Error(event.message)
                    }
                }
            } catch (t: Throwable) {
                _state.value = RouteUiState.Error("Couldn't reach the router.")
            }
        }
    }

    fun clear() {
        job?.cancel()
        _state.value = RouteUiState.Idle
    }

    fun saveAsTrack(name: String) {
        val plan = (_state.value as? RouteUiState.Done)?.plan ?: return
        if (plan.geometry.size < 2) return
        val geo = GeoPath(
            points = plan.geometry,
            source = GeoPathSource.Route,
            distanceM = plan.distanceM,
            ascentM = plan.ascentM,
            movingTimeSeconds = plan.durationS.toInt(),
            recordedAtEpochMs = System.currentTimeMillis(),
        )
        viewModelScope.launch {
            paths.save(SavedPath(id = "p-${UUID.randomUUID()}", name = name.ifBlank { "Route" }, path = geo))
        }
    }
}
