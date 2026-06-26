package com.sigmundgranaas.turbo.expressive.core.tracking

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

    /**
     * The shared capture engine (track + pause-buffer) — the SAME state machine
     * [FollowController] uses, so pause/resume behave identically in both modes (US-4).
     */
    private var capture = CaptureSession()

    fun start() {
        if (_session.value.active || !location.hasPermission()) return
        _session.value = RecordingSession(active = true)
        capture = CaptureSession()
        locationJob = scope.launch {
            // Resume any persisted draft (e.g. after a process kill) before collecting.
            draftStore.load()?.let { draft ->
                capture = CaptureSession.restore(draft.points, draft.elevations, draft.pausedFromIndex)
                publish { it.copy(elapsedSec = draft.elapsedSec) }
            }
            location.samples().collect { sample ->
                // Drop wildly inaccurate fixes (e.g. cold-start / indoor) before they pollute the track.
                if (!RecordingFilter.acceptAccuracy(sample.accuracyM)) return@collect
                if (!_session.value.active) return@collect
                capture = capture.fix(sample.position, sample.altitude, sample.speedMps)
                val updated = publish()
                // Persist the track (and, while paused, the held buffer past pausedFromIndex) so
                // a process death never loses the walk.
                if (capture.paused) {
                    draftStore.save(
                        capture.track.points + capture.buffer.points,
                        capture.track.elevations + capture.buffer.elevations,
                        updated.elapsedSec,
                        pausedFromIndex = capture.track.points.size,
                    )
                } else {
                    draftStore.save(updated.points, updated.elevations, updated.elapsedSec)
                }
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
        if (!_session.value.active || capture.paused) return
        capture = capture.pause()
        publish()
    }

    /**
     * Resume from a pause. [includeBuffered] = true stitches the walk captured while paused
     * onto the track (it IS the path you walked); false discards it and lifts the pen so the
     * gap isn't counted. Either way the buffer is cleared.
     */
    fun resume(includeBuffered: Boolean) {
        if (!_session.value.active || !capture.paused) return
        capture = capture.resume(includeBuffered)
        publish()
    }

    /**
     * One-tap pause/resume for the simple case. Pausing starts buffering; resuming this way
     * DISCARDS any buffer (the UI routes a significant buffer through [resume] with a prompt).
     */
    fun togglePause() {
        if (!_session.value.active) return
        if (capture.paused) resume(includeBuffered = false) else pause()
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
        capture = CaptureSession()
        _session.value = RecordingSession()
        scope.launch { draftStore.clear() }
    }

    /** Project the [capture] engine's fields into the public [session] flow (preserving active/elapsed). */
    private fun publish(extra: (RecordingSession) -> RecordingSession = { it }): RecordingSession =
        _session.updateAndGet { s ->
            extra(
                s.copy(
                    paused = capture.paused,
                    points = capture.points,
                    elevations = capture.elevations,
                    distanceM = capture.distanceM,
                    speedMps = capture.speedMps,
                    maxSpeedMps = capture.maxSpeedMps,
                    bufferedDistanceM = capture.bufferedDistanceM,
                ),
            )
        }
}

/** Pure sample-quality gating for recording, isolated for testability. */
internal object RecordingFilter {
    /** Horizontal-accuracy ceiling (m); fixes worse than this are dropped. */
    const val MAX_ACCURACY_M = 50.0

    /** Accept a fix unless its accuracy is known and worse than [MAX_ACCURACY_M]. */
    fun acceptAccuracy(accuracyM: Double?): Boolean = accuracyM == null || accuracyM <= MAX_ACCURACY_M
}
