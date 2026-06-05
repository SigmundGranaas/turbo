package com.sigmundgranaas.turbo.expressive.feature.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.LocationRepository
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

data class RecordingUiState(
    val hasPermission: Boolean = true,
    val recording: Boolean = false,
    val paused: Boolean = false,
    val points: List<LatLng> = emptyList(),
    val distanceM: Double = 0.0,
    val elapsedSec: Int = 0,
    val saved: Boolean = false,
) {
    val userLocation: LatLng? get() = points.lastOrNull()
}

/**
 * Real GPS track recording: collects fixes from [LocationRepository] while
 * recording (and not paused), accumulates distance + moving time, and saves the
 * finished track to [PathRepository] as a [GeoPath]. Foreground-only — recording
 * runs while the screen is open.
 */
@HiltViewModel
class RecordingViewModel @Inject constructor(
    private val location: LocationRepository,
    private val paths: PathRepository,
) : ViewModel() {
    private val _state = MutableStateFlow(RecordingUiState(hasPermission = location.hasPermission()))
    val state: StateFlow<RecordingUiState> = _state.asStateFlow()

    private var locationJob: Job? = null
    private var timerJob: Job? = null

    fun onPermissionResult(granted: Boolean) {
        _state.update { it.copy(hasPermission = granted) }
        if (granted && !_state.value.recording) start()
    }

    fun start() {
        if (_state.value.recording || !location.hasPermission()) return
        _state.update { it.copy(recording = true, paused = false, hasPermission = true) }
        locationJob = viewModelScope.launch {
            location.locationUpdates().collect { fix ->
                _state.update { s ->
                    if (!s.recording || s.paused) return@update s
                    if (s.points.isEmpty()) return@update s.copy(points = listOf(fix))
                    val step = GeoMetrics.haversineMeters(s.points.last(), fix)
                    if (step < MIN_STEP_M) return@update s
                    s.copy(points = s.points + fix, distanceM = s.distanceM + step)
                }
            }
        }
        timerJob = viewModelScope.launch {
            while (true) {
                delay(1_000)
                _state.update { if (it.recording && !it.paused) it.copy(elapsedSec = it.elapsedSec + 1) else it }
            }
        }
    }

    fun togglePause() = _state.update { it.copy(paused = !it.paused) }

    fun stop() {
        locationJob?.cancel()
        timerJob?.cancel()
        _state.update { it.copy(recording = false, paused = true) }
    }

    fun save(name: String, onSaved: () -> Unit) {
        val s = _state.value
        if (s.points.size < 2) { onSaved(); return }
        val geo = GeoPath.fromPoints(s.points, GeoPathSource.Recording).copy(
            movingTimeSeconds = s.elapsedSec,
            recordedAtEpochMs = System.currentTimeMillis(),
        )
        val safeName = name.ifBlank { "Track ${s.distanceM.toInt()} m" }
        viewModelScope.launch {
            paths.save(SavedPath(id = "p-${UUID.randomUUID()}", name = safeName, path = geo))
            _state.update { it.copy(saved = true) }
            onSaved()
        }
    }

    override fun onCleared() {
        locationJob?.cancel()
        timerJob?.cancel()
    }

    private companion object {
        /** Ignore jitter below this step (metres) so a stationary GPS doesn't inflate distance. */
        const val MIN_STEP_M = 3.0
    }
}
