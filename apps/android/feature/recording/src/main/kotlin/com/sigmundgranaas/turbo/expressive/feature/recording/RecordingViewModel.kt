package com.sigmundgranaas.turbo.expressive.feature.recording

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.LocationRepository
import com.sigmundgranaas.turbo.expressive.core.data.PathRepository
import com.sigmundgranaas.turbo.expressive.core.data.RecordingController
import com.sigmundgranaas.turbo.expressive.core.data.RecordingSession
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPathSource
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.SavedPath
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
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
) {
    val userLocation: LatLng? get() = points.lastOrNull()
}

/**
 * Thin UI driver over the process-lifetime [RecordingController]: the recording
 * itself runs in the foreground [RecordingService], so leaving this screen does
 * not stop it. The ViewModel mirrors the controller's session, starts/stops the
 * service, and persists the finished track.
 */
@HiltViewModel
class RecordingViewModel @Inject constructor(
    private val launcher: RecordingLauncher,
    private val controller: RecordingController,
    private val location: LocationRepository,
    private val paths: PathRepository,
) : ViewModel() {

    private val local = MutableStateFlow(RecordingUiState(hasPermission = location.hasPermission()))

    val state: StateFlow<RecordingUiState> = combine(controller.session, local) { session, ui ->
        ui.copy(
            recording = session.active,
            paused = session.paused,
            points = session.points,
            distanceM = session.distanceM,
            elapsedSec = session.elapsedSec,
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), local.value)

    /** The full recording session (elevations, speed) the live sheet renders from. */
    val session: StateFlow<RecordingSession> = controller.session

    fun onPermissionResult(granted: Boolean) {
        local.update { it.copy(hasPermission = granted) }
        if (granted && !controller.session.value.active) start()
    }

    /** Start (or no-op if already running) the foreground recording service. */
    fun start() {
        if (!location.hasPermission()) return
        launcher.start()
    }

    fun togglePause() = controller.togglePause()

    /** Pause; capture keeps buffering while paused (US-4). */
    fun pause() = controller.pause()

    /** Resume from a pause, [include]ing or discarding the walk captured while paused (US-4). */
    fun resume(include: Boolean) = controller.resume(include)

    fun stop() {
        launcher.stop()
    }

    fun save(name: String, kind: ActivityKindId? = null, onSaved: () -> Unit) {
        val session = controller.session.value
        if (session.points.size < 2) { controller.reset(); onSaved(); return }
        // Only attach elevations when at least one fix actually carried altitude.
        val elevations = session.elevations.takeIf { e -> e.any { it != null } }
        val geo = GeoPath.fromPoints(session.points, GeoPathSource.Recording, elevations).copy(
            movingTimeSeconds = session.elapsedSec,
            recordedAtEpochMs = System.currentTimeMillis(),
        )
        val safeName = name.ifBlank { "Track ${session.distanceM.toInt()} m" }
        viewModelScope.launch {
            paths.save(SavedPath(id = "p-${UUID.randomUUID()}", name = safeName, path = geo, activityKind = kind))
            controller.reset()
            onSaved()
        }
    }

    fun discard(onDone: () -> Unit) {
        controller.reset()
        onDone()
    }
}
