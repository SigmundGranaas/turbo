package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.FollowController
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.data.RouteRepository
import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.core.map.OfflineTileManager
import com.sigmundgranaas.turbo.expressive.core.map.RouteCorridor
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.DownloadSpec
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
    private val follow: FollowController,
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

    // A waypoint is being dragged on the map. While true, re-solves are DEFERRED so the route
    // line can't thrash under the user's finger (e.g. a follow-mode reroute landing mid-drag);
    // the deferred solve runs on [endWaypointDrag]. The drop's own commit ends the drag first,
    // so the moved waypoint re-solves normally.
    private var waypointDragActive = false
    private var solvePendingAfterDrag = false

    init {
        // Keep the route state in step with the follow session: if following ends from
        // outside (e.g. the lock-screen Stop button → FollowController.stop()), drop back
        // to Idle so the in-app surface and the widget can't disagree.
        viewModelScope.launch {
            follow.session.collect { s ->
                if (!s.active && _state.value is RouteUiState.Following) _state.value = RouteUiState.Idle
            }
        }
    }

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

    /**
     * Extend the route by appending a waypoint to the END (the new destination; the old
     * destination becomes a via). This is what sequential map taps do — predictable
     * order — as opposed to [addStop], which inserts a POI at the least-detour position.
     */
    fun appendWaypoint(point: LatLng) {
        val current = _waypoints.value
        if (current.size < 2) return
        pushUndo()
        setWaypoints(current + point, debounce = true)
    }

    /** Remove the waypoint at [index] (a route needs at least two; otherwise clears). */
    fun removeWaypoint(index: Int) {
        val current = _waypoints.value
        if (index !in current.indices) return
        pushUndo()
        val next = current.toMutableList().apply { removeAt(index) }
        if (next.size < 2) clear() else setWaypoints(next, debounce = true)
    }

    /** A map drag of a waypoint began — defer re-solves until it ends (see [waypointDragActive]). */
    fun beginWaypointDrag() {
        waypointDragActive = true
    }

    /** The map drag ended — resume re-solving, running any solve deferred during the drag. */
    fun endWaypointDrag() {
        waypointDragActive = false
        if (solvePendingAfterDrag) {
            solvePendingAfterDrag = false
            solve(debounce = true)
        }
    }

    /** Move the waypoint at [index] to a new position (drag on the map), then re-solve. */
    fun moveWaypointTo(index: Int, point: LatLng) {
        val current = _waypoints.value
        if (index !in current.indices) return
        pushUndo()
        setWaypoints(current.toMutableList().also { it[index] = point }, debounce = true)
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
        // Mid-drag: hold off. The waypoint state is already updated for the live overlay; the
        // re-solve runs once the finger lifts ([endWaypointDrag]) so the line doesn't thrash.
        if (waypointDragActive) {
            solvePendingAfterDrag = true
            return
        }
        job?.cancel()
        val current = _state.value
        // An off-route reroute while FOLLOWING must be INVISIBLE: re-solving in the background
        // and only swapping the route LINE in when the new plan lands. Dropping to Solving
        // here would tear down the live follow sheet, reset the map's bottom inset and un-dim
        // the covered line — a jarring view change mid-navigation. So while following we keep
        // showing Following(oldPlan) for the whole re-solve and ignore intermediate progress.
        // [resumeFollowing] is set by [reroute]; a first-time follow or a stop edit is unaffected.
        val silentFollow = resumeFollowing && current is RouteUiState.Following
        // Graceful re-route for a solved/following route: keep the current line on screen until
        // the new result lands, so the path doesn't snap to a straight line and back. Only the
        // very first solve (no prior geometry) shows the live straight-line→refined progress.
        val previous = when (current) {
            is RouteUiState.Done -> current.plan.geometry
            is RouteUiState.Following -> current.plan.geometry
            else -> null
        }
        if (!silentFollow) _state.value = RouteUiState.Solving(previous ?: points)
        job = viewModelScope.launch {
            if (debounce) kotlinx.coroutines.delay(DEBOUNCE_MS)
            try {
                routes.planStream(points, _preset.value).collect { event ->
                    when (event) {
                        is RouteStreamEvent.Progress ->
                            if (!silentFollow) {
                                _state.value =
                                    if (previous != null) RouteUiState.Solving(previous)
                                    else RouteUiState.Solving(event.coordinates)
                            } // following: stay on the old line, no flicker
                        is RouteStreamEvent.Result ->
                            _state.value =
                                if (resumeFollowing) { resumeFollowing = false; RouteUiState.Following(event.plan) }
                                else RouteUiState.Done(event.plan)
                        is RouteStreamEvent.Failure ->
                            // A failed reroute must not blow away the live view — keep following
                            // the existing line; only a normal solve surfaces the error.
                            if (!silentFollow) _state.value = RouteUiState.Error(event.message) else resumeFollowing = false
                    }
                }
            } catch (t: Throwable) {
                if (!silentFollow) _state.value = RouteUiState.Error("Couldn't reach the router.") else resumeFollowing = false
            }
        }
    }

    /** Re-solve from the current position to the same destination, staying in Follow mode. */
    fun reroute(from: LatLng) {
        val current = _waypoints.value
        if (current.size < 2 || _state.value !is RouteUiState.Following) return
        // One reroute at a time: the off-route check keeps seeing Following (the view never
        // leaves follow mode), so without this guard each new fix while off-route would pile
        // on another re-solve.
        if (resumeFollowing) return
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

    fun follow(nearbyCheckpoints: List<Pair<LatLng, String>> = emptyList()) {
        (_state.value as? RouteUiState.Done)?.let {
            _state.value = RouteUiState.Following(it.plan)
            // Checkpoints = the stops after the origin (US-3), labelled by their on-map letter
            // (B, C, …) so the split list matches the waypoint badges. Saved markers that sit near
            // the route are folded in as extra checkpoints (D3), then everything is ordered by
            // arc-length along the route so splits fire in travel order regardless of source.
            val stops = _waypoints.value.drop(1)
            val stopCps = stops.mapIndexed { i, p -> p to ('B' + i).toString() }
            val merged = (stopCps + nearbyCheckpoints)
                .sortedBy { (pos, _) -> GeoMetrics.arcLengthAlong(it.plan.geometry, pos) }
            follow.start(
                it.plan,
                phasePoints = merged.map { (pos, _) -> pos },
                phaseNames = merged.map { (_, name) -> name },
            )
        }
    }

    /** The live follow session backing the lock-screen widget; same source the sheet reads. */
    val followSession get() = follow.session

    /** Pause the follow — capture keeps buffering (US-4, Follow = Record). */
    fun pauseFollow() = follow.pause()

    /** Resume a paused follow; [include] stitches the paused-buffer walk onto the track (US-4). */
    fun resumeFollow(include: Boolean) = follow.resume(include)

    /** One-tap pause/resume (resuming this way discards the buffer; the UI prompts a big one). */
    fun toggleFollowPause() = follow.togglePause()

    /**
     * Follow an already-saved track's geometry (no solve) — used by "Follow" on a
     * saved track opened on the map, so following works for recorded/imported lines
     * the same as for planned routes.
     */
    fun followTrack(geometry: List<LatLng>, distanceM: Double, ascentM: Double, durationS: Double, name: String? = null) {
        if (geometry.size < 2) return
        job?.cancel()
        val plan = RoutePlan(
            distanceM = distanceM,
            durationS = durationS,
            ascentM = ascentM,
            onTrailPct = 0.0,
            surfaces = emptyMap(),
            geometry = geometry,
        )
        _state.value = RouteUiState.Following(plan)
        follow.start(plan, name = name)
    }

    fun clear() {
        job?.cancel()
        follow.stop()
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
        offline.download(DownloadSpec(name = name, base = base, bounds = bounds, minZoom = 8.0, maxZoom = 15.0))
    }

    fun saveAsTrack(name: String, kind: com.sigmundgranaas.turbo.expressive.domain.ActivityKindId? = null) {
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
            paths.save(SavedPath(id = "p-${UUID.randomUUID()}", name = name.ifBlank { "Route" }, path = geo, activityKind = kind))
        }
    }

    /**
     * Save a hand-built line (Create-track Line/Draw modes) as a track. Distance
     * is derived from the geometry; no solve, no surfaces.
     */
    fun saveLine(name: String, geometry: List<LatLng>) {
        if (geometry.size < 2) return
        val geo = GeoPath.fromPoints(geometry, GeoPathSource.Measure)
            .copy(recordedAtEpochMs = System.currentTimeMillis())
        viewModelScope.launch {
            paths.save(SavedPath(id = "p-${UUID.randomUUID()}", name = name.ifBlank { "Track" }, path = geo))
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
