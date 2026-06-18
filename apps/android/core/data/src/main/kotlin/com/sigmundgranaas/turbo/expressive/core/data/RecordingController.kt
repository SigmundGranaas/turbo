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
    /**
     * Distance (m) of movement captured *while paused* and not yet folded into the track
     * (US-4). The UI watches this to nudge "you're moving while paused" and to ask
     * Include/Discard on resume; 0 when there's nothing buffered.
     */
    val bufferedDistanceM: Double = 0.0,
) {
    val userLocation: LatLng? get() = points.lastOrNull()

    /** Whether enough was walked while paused to be worth asking about on resume. */
    val hasBufferedMovement: Boolean get() = bufferedDistanceM >= RESUME_PROMPT_M
}

/** Buffered walking past this (m) prompts Include/Discard on resume; below it, just resume. */
const val RESUME_PROMPT_M = 25.0

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

    /** Movement captured while paused, held pending Include/Discard on resume (US-4). */
    private var pausedCapture = CapturedTrack()
    /** The track point at the moment of pausing — buffered distance is measured from here. */
    private var pauseAnchor: LatLng? = null
    /** Set by a Discard resume so the next live fix starts a fresh segment (no gap distance). */
    private var penUpNext = false

    fun start() {
        if (_session.value.active || !location.hasPermission()) return
        _session.value = RecordingSession(active = true)
        pausedCapture = CapturedTrack(); pauseAnchor = null; penUpNext = false
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
                val s0 = _session.value
                if (!s0.active) return@collect
                if (s0.paused) {
                    // US-4: keep capturing while paused into a side buffer (so a forgotten
                    // unpause doesn't silently lose the walk), but leave the track untouched.
                    // Buffered distance is measured from the pause anchor so the first fix counts.
                    pausedCapture = TrackCapture.append(pausedCapture, sample.position, sample.altitude, sample.speedMps)
                    val join = pauseAnchor?.let { a ->
                        pausedCapture.points.firstOrNull()?.let { GeoMetrics.haversineMeters(a, it) }
                    } ?: 0.0
                    _session.update { it.copy(bufferedDistanceM = join + pausedCapture.distanceM) }
                    return@collect
                }
                val updated = _session.updateAndGet { s ->
                    if (!s.active || s.paused) return@updateAndGet s
                    val cur = CapturedTrack(s.points, s.elevations, s.distanceM, s.speedMps, s.maxSpeedMps)
                    // The same shared capture engine that powers Follow = Record. After a
                    // Discard resume the first fix is detached so the gap isn't counted.
                    val next = if (penUpNext) {
                        penUpNext = false
                        TrackCapture.appendDetached(cur, sample.position, sample.altitude, sample.speedMps)
                    } else {
                        TrackCapture.append(cur, sample.position, sample.altitude, sample.speedMps)
                    }
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

    /** Pause the recording; capture continues into a side buffer (US-4). */
    fun pause() {
        val s = _session.value
        if (!s.active || s.paused) return
        pauseAnchor = s.points.lastOrNull()
        _session.update { if (it.active && !it.paused) it.copy(paused = true) else it }
    }

    /**
     * Resume from a pause. [includeBuffered] = true stitches the walk captured while paused
     * onto the track (it IS the path you walked); false discards it and lifts the pen so the
     * gap isn't counted. Either way the buffer is cleared.
     */
    fun resume(includeBuffered: Boolean) {
        val buffered = pausedCapture
        _session.update { s ->
            if (!s.active || !s.paused) return@update s
            if (includeBuffered && buffered.points.isNotEmpty()) {
                val join = if (s.points.isNotEmpty()) {
                    GeoMetrics.haversineMeters(s.points.last(), buffered.points.first())
                } else {
                    0.0
                }
                s.copy(
                    paused = false,
                    points = s.points + buffered.points,
                    elevations = s.elevations + buffered.elevations,
                    distanceM = s.distanceM + join + buffered.distanceM,
                    maxSpeedMps = maxOf(s.maxSpeedMps, buffered.maxSpeedMps),
                    bufferedDistanceM = 0.0,
                )
            } else {
                penUpNext = true
                s.copy(paused = false, bufferedDistanceM = 0.0)
            }
        }
        pausedCapture = CapturedTrack(); pauseAnchor = null
    }

    /**
     * One-tap pause/resume for the simple case. Pausing starts buffering; resuming this way
     * DISCARDS any buffer (the UI routes a significant buffer through [resume] with a prompt).
     */
    fun togglePause() {
        val s = _session.value
        if (!s.active) return
        if (s.paused) resume(includeBuffered = false) else pause()
    }

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
