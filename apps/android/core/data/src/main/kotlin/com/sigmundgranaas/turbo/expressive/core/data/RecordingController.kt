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
    /** Latest instantaneous ground speed (m/s); null until a fix carries speed. */
    val speedMps: Double? = null,
    /** Fastest instantaneous speed seen this session (m/s). */
    val maxSpeedMps: Double = 0.0,
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
                // Drop wildly inaccurate fixes (e.g. cold-start / indoor) before they pollute the track.
                if (!RecordingFilter.acceptAccuracy(sample.accuracyM)) return@collect
                val updated = _session.updateAndGet { s ->
                    if (!s.active || s.paused) return@updateAndGet s
                    // The same shared capture engine that powers Follow = Record.
                    val next = TrackCapture.append(
                        CapturedTrack(s.points, s.elevations, s.distanceM, s.speedMps, s.maxSpeedMps),
                        sample.position, sample.altitude, sample.speedMps,
                    )
                    s.copy(
                        points = next.points, elevations = next.elevations,
                        distanceM = next.distanceM, speedMps = next.speedMps, maxSpeedMps = next.maxSpeedMps,
                    )
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
}

/** Pure sample-quality gating for recording, isolated for testability. */
internal object RecordingFilter {
    /** Horizontal-accuracy ceiling (m); fixes worse than this are dropped. */
    const val MAX_ACCURACY_M = 50.0

    /** Accept a fix unless its accuracy is known and worse than [MAX_ACCURACY_M]. */
    fun acceptAccuracy(accuracyM: Double?): Boolean = accuracyM == null || accuracyM <= MAX_ACCURACY_M
}
