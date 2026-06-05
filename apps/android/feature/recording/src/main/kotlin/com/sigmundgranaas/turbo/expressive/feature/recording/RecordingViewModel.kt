package com.sigmundgranaas.turbo.expressive.feature.recording

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import javax.inject.Inject

data class RecordingUiState(
    val paused: Boolean = false,
    val elapsed: String = "00:42:18",
    val distanceKm: String = "6.2",
    val ascentM: String = "480",
    val pace: String = "5:48",
    val progress: Float = 0.62f,
)

/** Recording state holder — `paused` is real session state, not ephemeral UI. */
@HiltViewModel
class RecordingViewModel @Inject constructor() : ViewModel() {
    private val _state = MutableStateFlow(RecordingUiState())
    val state: StateFlow<RecordingUiState> = _state.asStateFlow()

    fun togglePause() = _state.update { it.copy(paused = !it.paused) }
}
