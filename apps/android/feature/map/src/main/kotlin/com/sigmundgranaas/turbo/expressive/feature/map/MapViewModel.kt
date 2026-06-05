package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.data.LocationRepository
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
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
    val following: Boolean = false,
    val userLocation: LatLng? = null,
)

/** Holds the map home's UI state; markers + live location come from repositories. */
@HiltViewModel
class MapViewModel @Inject constructor(
    private val markerRepository: MarkerRepository,
    private val location: LocationRepository,
) : ViewModel() {
    private val _state = MutableStateFlow(MapUiState())
    val state: StateFlow<MapUiState> = _state.asStateFlow()

    private var locationJob: Job? = null

    init {
        viewModelScope.launch {
            markerRepository.observeAll().collect { markers ->
                _state.update { it.copy(markers = markers) }
            }
        }
    }

    fun hasLocationPermission(): Boolean = location.hasPermission()

    /** Begin streaming the user's location into [MapUiState.userLocation]. Idempotent. */
    fun enableLocation() {
        if (locationJob != null || !location.hasPermission()) return
        locationJob = viewModelScope.launch {
            location.locationUpdates().collect { fix ->
                _state.update { it.copy(userLocation = fix) }
            }
        }
    }

    fun setBaseLayer(layer: BaseLayer) = _state.update { it.copy(baseLayer = layer) }
    fun setFollowing(value: Boolean) = _state.update { it.copy(following = value) }

    /** Persist a new user marker (offline-first; the map updates via [observeAll]). */
    fun addMarker(
        name: String,
        kind: ActivityKindId,
        position: LatLng,
        colorArgb: Long? = null,
        notes: String? = null,
    ) {
        val safeName = name.ifBlank { kind.label }
        viewModelScope.launch {
            markerRepository.upsert(
                Marker(
                    id = "m-${UUID.randomUUID()}",
                    name = safeName,
                    kind = kind,
                    position = position,
                    colorArgb = colorArgb,
                    notes = notes,
                ),
            )
        }
    }

    /** Persist edits to an existing marker (same id → Room replaces the row). */
    fun updateMarker(marker: Marker) {
        viewModelScope.launch { markerRepository.upsert(marker) }
    }

    fun deleteMarker(id: String) {
        viewModelScope.launch { markerRepository.delete(id) }
    }
}
