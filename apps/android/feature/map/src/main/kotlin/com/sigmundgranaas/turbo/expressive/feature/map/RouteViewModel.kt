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
    data class Following(val plan: RoutePlan) : RouteUiState
    data class Error(val message: String) : RouteUiState

    /** The polyline to draw for this state (empty when nothing to show). */
    val polyline: List<LatLng>
        get() = when (this) {
            is Solving -> progress
            is Done -> plan.geometry
            is Following -> plan.geometry
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

    private val _preset = MutableStateFlow(RoutePreset.Balanced)
    val preset: StateFlow<RoutePreset> = _preset.asStateFlow()

    private var job: Job? = null
    private var from: LatLng? = null
    private var to: LatLng? = null
    private var resumeFollowing = false

    fun planRoute(from: LatLng, to: LatLng, preset: RoutePreset = _preset.value) {
        this.from = from
        this.to = to
        _preset.value = preset
        job?.cancel()
        // Seed with the straight line so the user sees intent immediately.
        _state.value = RouteUiState.Solving(listOf(from, to))
        job = viewModelScope.launch {
            try {
                routes.planStream(listOf(from, to), preset).collect { event ->
                    _state.value = when (event) {
                        is RouteStreamEvent.Progress -> RouteUiState.Solving(event.coordinates)
                        is RouteStreamEvent.Result ->
                            if (resumeFollowing) { resumeFollowing = false; RouteUiState.Following(event.plan) }
                            else RouteUiState.Done(event.plan)
                        is RouteStreamEvent.Failure -> RouteUiState.Error(event.message)
                    }
                }
            } catch (t: Throwable) {
                _state.value = RouteUiState.Error("Couldn't reach the router.")
            }
        }
    }

    /** Re-solve from the current position to the same destination, staying in Follow mode. */
    fun reroute(from: LatLng) {
        val dest = to ?: return
        if (_state.value !is RouteUiState.Following) return
        resumeFollowing = true
        planRoute(from, dest, _preset.value)
    }

    /** Re-plan the current trip with a different style. */
    fun selectPreset(preset: RoutePreset) {
        val origin = from
        val dest = to
        _preset.value = preset
        if (origin != null && dest != null) planRoute(origin, dest, preset)
    }

    /** Enter turn-by-route following for a solved route. */
    fun follow() {
        (_state.value as? RouteUiState.Done)?.let { _state.value = RouteUiState.Following(it.plan) }
    }

    fun clear() {
        job?.cancel()
        from = null
        to = null
        _state.value = RouteUiState.Idle
    }

    fun saveAsTrack(name: String) {
        val plan = when (val s = _state.value) {
            is RouteUiState.Done -> s.plan
            is RouteUiState.Following -> s.plan
            else -> return
        }
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
