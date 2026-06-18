package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.geo.RouteProgress
import com.sigmundgranaas.turbo.expressive.core.geo.RouteProgressTracker
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The live follow session — the route the user is actively walking along, with
 * progress projected from their current GPS position. Process-lifetime, like
 * [RecordingSession], so the background service can surface a lock-screen Live
 * Update fed by the very same state the in-app sheet reads (they can't drift).
 */
data class FollowSession(
    val active: Boolean = false,
    /** The route being followed (geometry + distance/ascent), or null when idle. */
    val plan: RoutePlan? = null,
    /** A short label for the route (e.g. its saved name); null when unnamed. */
    val name: String? = null,
    /** Latest fix along the route. */
    val position: LatLng? = null,
    /** Live arc-length-cursor progress, recomputed on every fix. */
    val progress: RouteProgress? = null,
    /** Latest instantaneous ground speed (m/s); null until a fix carries speed. */
    val speedMps: Double? = null,
) {
    /** Whether the hiker has effectively reached the end of the route. */
    val arrived: Boolean get() = active && (progress?.arrived ?: false)
}

/**
 * App-scoped follow engine: holds the followed route and projects the live GPS
 * position onto it (fraction done, distance/ETA remaining, off-route, arrived)
 * via the monotonic arc-length [RouteProgressTracker] — correct on loops /
 * out-and-back, unlike the old global-nearest projection. A [Singleton] so a
 * follow survives the UI being backgrounded; the foreground service keeps the
 * process alive.
 */
@Singleton
class FollowController @Inject constructor(
    private val location: LocationRepository,
    private val scope: CoroutineScope,
) {
    private val _session = MutableStateFlow(FollowSession())
    val session: StateFlow<FollowSession> = _session.asStateFlow()

    private var locationJob: Job? = null
    private var tracker: RouteProgressTracker? = null

    fun start(plan: RoutePlan, name: String? = null) {
        _session.value = FollowSession(active = true, plan = plan, name = name)
        tracker = RouteProgressTracker(route = plan.geometry, ascentM = plan.ascentM)
        if (!location.hasPermission()) return
        locationJob?.cancel()
        locationJob = scope.launch {
            location.samples().collect { sample ->
                val progress = tracker?.update(sample.position)
                _session.update { s ->
                    if (s.plan == null) return@update s
                    s.copy(
                        position = sample.position,
                        progress = progress,
                        speedMps = sample.speedMps ?: s.speedMps,
                    )
                }
            }
        }
    }

    fun stop() {
        locationJob?.cancel(); locationJob = null
        tracker = null
        _session.value = FollowSession()
    }
}
