package com.sigmundgranaas.turbo.expressive.core.data

import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/** The live recording session — process-lifetime, independent of any screen. */
data class RecordingSession(
    val active: Boolean = false,
    val paused: Boolean = false,
    val points: List<LatLng> = emptyList(),
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
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val _session = MutableStateFlow(RecordingSession())
    val session: StateFlow<RecordingSession> = _session.asStateFlow()

    private var locationJob: Job? = null
    private var timerJob: Job? = null

    fun start() {
        if (_session.value.active || !location.hasPermission()) return
        _session.value = RecordingSession(active = true)
        locationJob = scope.launch {
            location.locationUpdates().collect { fix ->
                _session.update { s ->
                    if (!s.active || s.paused) return@update s
                    if (s.points.isEmpty()) return@update s.copy(points = listOf(fix))
                    val step = GeoMetrics.haversineMeters(s.points.last(), fix)
                    if (step < MIN_STEP_M) return@update s
                    s.copy(points = s.points + fix, distanceM = s.distanceM + step)
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
    }

    private companion object {
        const val MIN_STEP_M = 3.0
    }
}
