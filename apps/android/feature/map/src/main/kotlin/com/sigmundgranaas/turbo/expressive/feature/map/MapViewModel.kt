package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

data class MapUiState(
    val markers: List<Marker> = emptyList(),
    val baseLayer: BaseLayer = BaseLayer.Norgeskart,
    val overlays: Set<OverlayId> = setOf(OverlayId.Waves),
    val following: Boolean = false,
    val compassOn: Boolean = false,
)

/** Holds the map home's UI state; markers come from the offline-first repository. */
@HiltViewModel
class MapViewModel @Inject constructor(
    private val markerRepository: MarkerRepository,
) : ViewModel() {
    private val _state = MutableStateFlow(MapUiState())
    val state: StateFlow<MapUiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            markerRepository.observeAll().collect { markers ->
                _state.update { it.copy(markers = markers) }
            }
        }
    }

    fun setBaseLayer(layer: BaseLayer) = _state.update { it.copy(baseLayer = layer) }
    fun toggleOverlay(id: OverlayId) = _state.update {
        it.copy(overlays = if (id in it.overlays) it.overlays - id else it.overlays + id)
    }
    fun toggleFollowing() = _state.update { it.copy(following = !it.following) }
    fun toggleCompass() = _state.update { it.copy(compassOn = !it.compassOn) }

    /** Persist a new user marker (offline-first; the map updates via [observeAll]). */
    fun addMarker(name: String, kind: ActivityKindId, position: LatLng) {
        val safeName = name.ifBlank { kind.label }
        viewModelScope.launch {
            markerRepository.upsert(
                Marker(id = "m-${UUID.randomUUID()}", name = safeName, kind = kind, position = position),
            )
        }
    }

    fun deleteMarker(id: String) {
        viewModelScope.launch { markerRepository.delete(id) }
    }
}
