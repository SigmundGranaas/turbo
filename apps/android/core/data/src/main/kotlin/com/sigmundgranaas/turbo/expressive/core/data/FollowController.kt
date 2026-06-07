package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.core.geo.JourneyProgress
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
    /** Live progress projection, recomputed on every fix. */
    val progress: JourneyProgress? = null,
    /** Latest instantaneous ground speed (m/s); null until a fix carries speed. */
    val speedMps: Double? = null,
) {
    /** Whether the hiker has effectively reached the end of the route. */
    val arrived: Boolean get() = active && (progress?.distanceRemainingM ?: Double.MAX_VALUE) <= ARRIVAL_M

    private companion object {
        const val ARRIVAL_M = 40.0
    }
}

/**
 * App-scoped follow engine: holds the followed route and projects the live GPS
 * position onto it (fraction done, distance/ETA remaining) via the same pure
 * [GeoMetrics.progress] the UI uses. A [Singleton] so a follow survives the UI
 * being backgrounded; the foreground service keeps the process alive.
 */
@Singleton
class FollowController @Inject constructor(
    private val location: LocationRepository,
    private val scope: CoroutineScope,
) {
    private val _session = MutableStateFlow(FollowSession())
    val session: StateFlow<FollowSession> = _session.asStateFlow()

    private var locationJob: Job? = null

    fun start(plan: RoutePlan, name: String? = null) {
        _session.value = FollowSession(active = true, plan = plan, name = name)
        if (!location.hasPermission()) return
        locationJob?.cancel()
        locationJob = scope.launch {
            location.samples().collect { sample ->
                _session.update { s ->
                    val p = s.plan ?: return@update s
                    s.copy(
                        position = sample.position,
                        progress = GeoMetrics.progress(p.geometry, sample.position, p.ascentM),
                        speedMps = sample.speedMps ?: s.speedMps,
                    )
                }
            }
        }
    }

    fun stop() {
        locationJob?.cancel(); locationJob = null
        _session.value = FollowSession()
    }
}
