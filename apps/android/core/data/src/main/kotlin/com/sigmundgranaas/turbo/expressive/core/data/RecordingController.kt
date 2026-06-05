package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.flow.updateAndGet
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/** The live recording session — process-lifetime, independent of any screen. */
data class RecordingSession(
    val active: Boolean = false,
    val paused: Boolean = false,
    val points: List<LatLng> = emptyList(),
    /** Altitude (m) per point; parallel to [points]. Null entries = fix had no altitude. */
    val elevations: List<Double?> = emptyList(),
    val distanceM: Double = 0.0,
    val elapsedSec: Int = 0,
) {
    val userLocation: LatLng? get() = points.lastOrNull()
}

/**
 * App-scoped recording engine: collects GPS fixes, accumulates distance + moving
 * time, and exposes one [session] flow. Lives for the whole process (a
 * [Singleton]) so a [com.sigmundgranaas.turbo.expressive.core.data.LocationRepository]-fed
 * recording keeps running while the UI is backgrounded — the foreground service
 * keeps the process alive; this holds the data.
 */
@Singleton
class RecordingController @Inject constructor(
    private val location: LocationRepository,
    private val draftStore: RecordingDraftStore,
    private val scope: CoroutineScope,
) {
    private val _session = MutableStateFlow(RecordingSession())
    val session: StateFlow<RecordingSession> = _session.asStateFlow()

    private var locationJob: Job? = null
    private var timerJob: Job? = null

    fun start() {
        if (_session.value.active || !location.hasPermission()) return
        _session.value = RecordingSession(active = true)
        locationJob = scope.launch {
            // Resume any persisted draft (e.g. after a process kill) before collecting.
            draftStore.load()?.let { draft ->
                _session.update {
                    it.copy(
                        points = draft.points,
                        elevations = draft.elevations,
                        distanceM = GeoMetrics.pathLengthMeters(draft.points),
                        elapsedSec = draft.elapsedSec,
                    )
                }
            }
            location.samples().collect { sample ->
                val fix = sample.position
                val updated = _session.updateAndGet { s ->
                    if (!s.active || s.paused) return@updateAndGet s
                    if (s.points.isEmpty()) return@updateAndGet s.copy(points = listOf(fix), elevations = listOf(sample.altitude))
                    val step = GeoMetrics.haversineMeters(s.points.last(), fix)
                    if (step < MIN_STEP_M) return@updateAndGet s
                    s.copy(points = s.points + fix, elevations = s.elevations + sample.altitude, distanceM = s.distanceM + step)
                }
                // Persist the growing track so it survives process death.
                draftStore.save(updated.points, updated.elevations, updated.elapsedSec)
            }
        }
        timerJob = scope.launch {
            while (true) {
                delay(1_000)
                _session.update { if (it.active && !it.paused) it.copy(elapsedSec = it.elapsedSec + 1) else it }
            }
        }
    }

    fun togglePause() = _session.update { if (it.active) it.copy(paused = !it.paused) else it }

    /** Stop collecting; keeps the captured session so the UI can offer "save". */
    fun stop() {
        locationJob?.cancel(); locationJob = null
        timerJob?.cancel(); timerJob = null
        _session.update { it.copy(active = false, paused = true) }
    }

    /** Clear the session after it has been saved or discarded. */
    fun reset() {
        stop()
        _session.value = RecordingSession()
        scope.launch { draftStore.clear() }
    }

    private companion object {
        const val MIN_STEP_M = 3.0
    }
}
