package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.core.map.OfflineTileManager
import com.sigmundgranaas.turbo.expressive.core.map.RouteCorridor
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
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
    private val offline: OfflineTileManager,
) : ViewModel() {
    private val _state = MutableStateFlow<RouteUiState>(RouteUiState.Idle)
    val state: StateFlow<RouteUiState> = _state.asStateFlow()

    private val _preset = MutableStateFlow(RoutePreset.Balanced)
    val preset: StateFlow<RoutePreset> = _preset.asStateFlow()

    /** Ordered route waypoints: first is the origin, last the destination, the rest stops. */
    private val _waypoints = MutableStateFlow<List<LatLng>>(emptyList())
    val waypoints: StateFlow<List<LatLng>> = _waypoints.asStateFlow()

    private var job: Job? = null
    private var resumeFollowing = false
    private val undoStack = ArrayDeque<List<LatLng>>()

    /** Start a fresh two-point route (origin → destination). */
    fun planRoute(from: LatLng, to: LatLng, preset: RoutePreset = _preset.value) {
        _preset.value = preset
        undoStack.clear()
        setWaypoints(listOf(from, to), debounce = false)
    }

    /**
     * Add an intermediate stop, inserted at the least-detour position between
     * existing waypoints, then re-solve. No-op until a route exists.
     */
    fun addStop(point: LatLng) {
        val current = _waypoints.value
        if (current.size < 2) return
        pushUndo()
        setWaypoints(Waypoints.insertLeastDetour(current, point), debounce = true)
    }

    /** Remove the waypoint at [index] (a route needs at least two; otherwise clears). */
    fun removeWaypoint(index: Int) {
        val current = _waypoints.value
        if (index !in current.indices) return
        pushUndo()
        val next = current.toMutableList().apply { removeAt(index) }
        if (next.size < 2) clear() else setWaypoints(next, debounce = true)
    }

    /** Reorder a waypoint (drag), then re-solve. */
    fun moveWaypoint(from: Int, to: Int) {
        val current = _waypoints.value
        if (from !in current.indices || to !in current.indices || from == to) return
        pushUndo()
        val next = current.toMutableList().apply { add(to, removeAt(from)) }
        setWaypoints(next, debounce = true)
    }

    /** Revert the last waypoint edit. */
    fun undo() {
        val previous = undoStack.removeLastOrNull() ?: return
        if (previous.size < 2) clear() else setWaypoints(previous, debounce = false)
    }

    val canUndo: Boolean get() = undoStack.isNotEmpty()

    private fun pushUndo() {
        undoStack.addLast(_waypoints.value)
        if (undoStack.size > UNDO_LIMIT) undoStack.removeFirst()
    }

    private fun setWaypoints(points: List<LatLng>, debounce: Boolean) {
        _waypoints.value = points
        solve(debounce)
    }

    private fun solve(debounce: Boolean) {
        val points = _waypoints.value
        if (points.size < 2) return
        job?.cancel()
        // Seed with the straight line through all waypoints so intent shows immediately.
        _state.value = RouteUiState.Solving(points)
        job = viewModelScope.launch {
            if (debounce) kotlinx.coroutines.delay(DEBOUNCE_MS)
            try {
                routes.planStream(points, _preset.value).collect { event ->
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
        val current = _waypoints.value
        if (current.size < 2 || _state.value !is RouteUiState.Following) return
        resumeFollowing = true
        // Replace the origin with the live position, keep the stops + destination.
        setWaypoints(listOf(from) + current.drop(1), debounce = false)
    }

    /** Re-plan the current trip with a different style. */
    fun selectPreset(preset: RoutePreset) {
        _preset.value = preset
        if (_waypoints.value.size >= 2) solve(debounce = false)
    }

    /** Enter turn-by-route following for a solved route. */
    /** Load a saved track by id (for "open on map"). */
    suspend fun pathById(id: String): com.sigmundgranaas.turbo.expressive.domain.SavedPath? = paths.byId(id)

    fun follow() {
        (_state.value as? RouteUiState.Done)?.let { _state.value = RouteUiState.Following(it.plan) }
    }

    /**
     * Follow an already-saved track's geometry (no solve) — used by "Follow" on a
     * saved track opened on the map, so following works for recorded/imported lines
     * the same as for planned routes.
     */
    fun followTrack(geometry: List<LatLng>, distanceM: Double, ascentM: Double, durationS: Double) {
        if (geometry.size < 2) return
        job?.cancel()
        _state.value = RouteUiState.Following(
            RoutePlan(
                distanceM = distanceM,
                durationS = durationS,
                ascentM = ascentM,
                onTrailPct = 0.0,
                surfaces = emptyMap(),
                geometry = geometry,
            ),
        )
    }

    fun clear() {
        job?.cancel()
        _waypoints.value = emptyList()
        undoStack.clear()
        _state.value = RouteUiState.Idle
    }

    /** Queue an offline download of the padded corridor around the solved route. */
    fun downloadAlongRoute(base: BaseLayer, name: String = "Route area") {
        val geometry = when (val s = _state.value) {
            is RouteUiState.Done -> s.plan.geometry
            is RouteUiState.Following -> s.plan.geometry
            else -> return
        }
        val bounds = RouteCorridor.bounds(geometry) ?: return
        offline.download(name, base, bounds, minZoom = 8.0, maxZoom = 15.0)
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

    private companion object {
        const val DEBOUNCE_MS = 300L
        const val UNDO_LIMIT = 20
    }
}

/** Pure waypoint geometry, isolated for testability. */
internal object Waypoints {
    /**
     * Insert [point] into [waypoints] at the position that adds the least extra
     * straight-line distance — i.e. on the segment whose detour through the point
     * is smallest. Endpoints (origin, destination) are never displaced.
     */
    fun insertLeastDetour(waypoints: List<LatLng>, point: LatLng): List<LatLng> {
        if (waypoints.size < 2) return waypoints + point
        var bestIndex = 1
        var bestDelta = Double.MAX_VALUE
        for (i in 0 until waypoints.size - 1) {
            val a = waypoints[i]
            val b = waypoints[i + 1]
            val delta = GeoMetrics.haversineMeters(a, point) +
                GeoMetrics.haversineMeters(point, b) -
                GeoMetrics.haversineMeters(a, b)
            if (delta < bestDelta) { bestDelta = delta; bestIndex = i + 1 }
        }
        return waypoints.toMutableList().apply { add(bestIndex, point) }
    }
}
