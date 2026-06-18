package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.core.geo.PhaseSplit
import com.sigmundgranaas.turbo.expressive.core.geo.RouteProgress
import com.sigmundgranaas.turbo.expressive.core.geo.RouteProgressTracker
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.RoutePlan
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton

/**
 * The live follow session — the route the user is actively walking along, with
 * progress projected from their current GPS position. Process-lifetime, like
 * [RecordingSession], so the background service can surface a lock-screen Live
 * Update fed by the very same state the in-app sheet reads (they can't drift).
 *
 * Follow = Record: while you follow a route this session ALSO captures the real
 * travelled track (the [points]/[elevations]/[capturedDistanceM]/[elapsedSec]
 * fields), via the same [TrackCapture] engine recording uses — so finishing a
 * follow auto-saves a genuine recording, not a thrown-away projection.
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
    // --- Follow = Record: the actual track captured while following ---
    /** The travelled polyline (NOT the planned route) captured from real fixes. */
    val points: List<LatLng> = emptyList(),
    /** Altitude (m) per captured point; parallel to [points]. */
    val elevations: List<Double?> = emptyList(),
    /** Cumulative travelled distance (m) — what you actually walked. */
    val capturedDistanceM: Double = 0.0,
    /** Fastest instantaneous speed seen (m/s). */
    val maxSpeedMps: Double = 0.0,
    /** Moving time (s) since the follow started. */
    val elapsedSec: Int = 0,
    /** Checkpoints crossed so far, with split times (US-3). */
    val phaseSplits: List<PhaseSplit> = emptyList(),
    /** The next checkpoint's name + distance to it, or null when all are crossed. */
    val nextPhaseName: String? = null,
    val nextPhaseDistanceM: Double? = null,
) {
    /** Whether the hiker has effectively reached the end of the route. */
    val arrived: Boolean get() = active && (progress?.arrived ?: false)
}

/**
 * App-scoped follow engine: holds the followed route, projects the live GPS
 * position onto it (fraction done, distance/ETA remaining, off-route, arrived)
 * via the monotonic arc-length [RouteProgressTracker] — correct on loops /
 * out-and-back, unlike the old global-nearest projection — AND captures the real
 * travelled track alongside (Follow = Record). On [stop] it **auto-saves** that
 * track (D1) unless it's too short to be worth keeping. A [Singleton] so a follow
 * survives the UI being backgrounded; the foreground service keeps the process alive.
 */
@Singleton
class FollowController @Inject constructor(
    private val location: LocationRepository,
    private val paths: PathRepository,
    private val scope: CoroutineScope,
) {
    private val _session = MutableStateFlow(FollowSession())
    val session: StateFlow<FollowSession> = _session.asStateFlow()

    private var locationJob: Job? = null
    private var timerJob: Job? = null
    private var tracker: RouteProgressTracker? = null
    // Phase (checkpoint) state (US-3): names parallel to the tracker's phase positions, plus the
    // captured distance/time at the last crossing (so each split is "since the previous phase").
    private var phaseNames: List<String> = emptyList()
    private var lastPhaseDistanceM = 0.0
    private var lastPhaseSec = 0

    /**
     * Begin following [plan]. [phasePoints] (typically the route's waypoints) + [phaseNames]
     * become checkpoints whose crossings are split-timed (US-3).
     */
    fun start(
        plan: RoutePlan,
        name: String? = null,
        phasePoints: List<LatLng> = emptyList(),
        phaseNames: List<String> = emptyList(),
    ) {
        _session.value = FollowSession(active = true, plan = plan, name = name)
        tracker = RouteProgressTracker(route = plan.geometry, ascentM = plan.ascentM, phasePositions = phasePoints)
        this.phaseNames = phaseNames
        lastPhaseDistanceM = 0.0
        lastPhaseSec = 0
        if (!location.hasPermission()) return
        locationJob?.cancel()
        locationJob = scope.launch {
            location.samples().collect { sample ->
                val t = tracker ?: return@collect
                val progress = t.update(sample.position)
                val s = _session.value
                if (s.plan == null) return@collect
                // Capture the travelled track with the SAME engine recording uses,
                // so the saved follow is byte-for-byte what a recording would be.
                val next = TrackCapture.append(
                    CapturedTrack(s.points, s.elevations, s.capturedDistanceM, s.speedMps, s.maxSpeedMps),
                    sample.position, sample.altitude, sample.speedMps,
                )
                // Record a split for each checkpoint the cursor just passed.
                var splits = s.phaseSplits
                if (t.passedPhaseCount > splits.size) {
                    val acc = splits.toMutableList()
                    for (i in splits.size until t.passedPhaseCount) {
                        acc += PhaseSplit(
                            index = i,
                            name = phaseNames.getOrElse(i) { "Checkpoint ${i + 1}" },
                            crossedAtEpochMs = System.currentTimeMillis(),
                            splitDistanceM = next.distanceM - lastPhaseDistanceM,
                            splitSeconds = s.elapsedSec - lastPhaseSec,
                        )
                        lastPhaseDistanceM = next.distanceM
                        lastPhaseSec = s.elapsedSec
                    }
                    splits = acc
                }
                val nextIdx = t.nextPhaseIndex
                val nextName = phaseNames.getOrNull(nextIdx)
                _session.value = s.copy(
                    position = sample.position,
                    progress = progress,
                    speedMps = sample.speedMps ?: s.speedMps,
                    points = next.points,
                    elevations = next.elevations,
                    capturedDistanceM = next.distanceM,
                    maxSpeedMps = next.maxSpeedMps,
                    phaseSplits = splits,
                    nextPhaseName = nextName,
                    nextPhaseDistanceM = if (nextName != null) t.distanceToPhase(nextIdx) else null,
                )
            }
        }
        timerJob?.cancel()
        timerJob = scope.launch {
            while (true) {
                delay(1_000)
                _session.update { if (it.active) it.copy(elapsedSec = it.elapsedSec + 1) else it }
            }
        }
    }

    /**
     * Stop following and AUTO-SAVE the travelled track (D1) — unless it's too short to
     * be worth keeping. What we persist is the real line walked, identical to a recording
     * of the same fixes; the planned route is untouched.
     */
    fun stop() {
        locationJob?.cancel(); locationJob = null
        timerJob?.cancel(); timerJob = null
        tracker = null
        autoSave(_session.value)
        _session.value = FollowSession()
    }

    private fun autoSave(s: FollowSession) {
        if (s.points.size < 2 || s.capturedDistanceM < MIN_SAVE_M) return
        // Only attach elevations when at least one fix actually carried altitude.
        val elevations = s.elevations.takeIf { e -> e.any { it != null } }
        val geo = GeoPath.fromPoints(s.points, GeoPathSource.Recording, elevations).copy(
            movingTimeSeconds = s.elapsedSec,
            recordedAtEpochMs = System.currentTimeMillis(),
        )
        val name = s.name?.let { "$it (followed)" } ?: "Followed route ${s.capturedDistanceM.toInt()} m"
        scope.launch {
            paths.save(SavedPath(id = "p-${UUID.randomUUID()}", name = name, path = geo, activityKind = null))
        }
    }

    private companion object {
        /** Skip auto-saving trivially short follows (you barely moved). */
        const val MIN_SAVE_M = 50.0
    }
}
