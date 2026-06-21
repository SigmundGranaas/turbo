package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.common.StringProvider
import com.sigmundgranaas.turbo.expressive.core.data.LocationRepository
import com.sigmundgranaas.turbo.expressive.core.data.MarkerRepository
import com.sigmundgranaas.turbo.expressive.core.data.RadarRepository
import com.sigmundgranaas.turbo.expressive.core.data.ReverseGeocodeRepository
import com.sigmundgranaas.turbo.expressive.core.data.SettingsRepository
import com.sigmundgranaas.turbo.expressive.feature.map.radar.MetRadarDataSource
import com.sigmundgranaas.turbo.expressive.feature.map.radar.RadarDataSource
import com.sigmundgranaas.turbo.expressive.domain.ActivityKindId
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.LocationDescription
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.ui.theme.labelRes
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject

/** Why an initial GPS centre couldn't happen — surfaced as a one-shot banner.
 *  Slow GPS is NOT a notice: the first fix just takes a while, so we wait
 *  quietly rather than nag. Only services-off (an actionable, persistent state)
 *  is surfaced. */
enum class LocationNotice { ServicesOff }

data class MapUiState(
    val markers: List<Marker> = emptyList(),
    val baseLayer: BaseLayer = BaseLayer.Norgeskart,
    val following: Boolean = false,
    val userLocation: LatLng? = null,
    /** True while we've started locating but no fix has arrived yet (drives a "locating…" hint). */
    val locating: Boolean = false,
    val locationNotice: LocationNotice? = null,
    /** Experimental: render with the wgpu engine instead of MapLibre (Settings toggle). */
    val experimentalWgpuMap: Boolean = false,
    /** Persisted camera from the last session — restored on open so the user
     *  returns to where they left off. Null until the map has been moved once. */
    val lastCamera: LatLng? = null,
    val lastCameraZoom: Double? = null,
    /**
     * 3D map mode (wgpu engine only): a 1-finger drag orbits about the user
     * location, two fingers pan. Off = the legacy 2D pan/zoom. Session state —
     * the toggle only appears when [experimentalWgpuMap] is on.
     */
    val threeDMode: Boolean = false,
)

/** Holds the map home's UI state; markers + live location come from repositories. */
@HiltViewModel
class MapViewModel @Inject constructor(
    private val markerRepository: MarkerRepository,
    private val location: LocationRepository,
    private val reverseGeocode: ReverseGeocodeRepository,
    private val strings: StringProvider,
    private val settings: SettingsRepository,
    radarRepository: RadarRepository,
) : ViewModel() {
    private val _state = MutableStateFlow(MapUiState())
    val state: StateFlow<MapUiState> = _state.asStateFlow()

    /** Live cloud-overlay source (real MET weather, synthetic fallback offline). */
    val radarSource: RadarDataSource = MetRadarDataSource(radarRepository)

    /** Reverse-geocoded description of the point a new marker is being dropped at. */
    private val _pointDescription = MutableStateFlow<LocationDescription?>(null)
    val pointDescription: StateFlow<LocationDescription?> = _pointDescription.asStateFlow()

    private var locationJob: Job? = null
    private var describeJob: Job? = null

    init {
        viewModelScope.launch {
            markerRepository.observeAll().collect { markers ->
                _state.update { it.copy(markers = markers) }
            }
        }
        // Restore (and keep in sync with) the persisted base map so the choice
        // survives relaunch instead of resetting to Norgeskart every time.
        viewModelScope.launch {
            settings.settings.collect { s ->
                _state.update {
                    it.copy(
                        baseLayer = s.baseLayer,
                        experimentalWgpuMap = s.experimentalWgpuMap,
                        lastCamera = if (s.lastCameraLat != null && s.lastCameraLng != null) {
                            LatLng(s.lastCameraLat!!, s.lastCameraLng!!)
                        } else {
                            null
                        },
                        lastCameraZoom = s.lastCameraZoom,
                    )
                }
            }
        }
    }

    fun hasLocationPermission(): Boolean = location.hasPermission()

    /** Begin streaming the user's location into [MapUiState.userLocation]. Idempotent. */
    fun enableLocation() {
        if (locationJob != null || !location.hasPermission()) return
        _state.update { it.copy(locating = true, locationNotice = null) }
        locationJob = viewModelScope.launch {
            location.locationUpdates().collect { fix ->
                _state.update { it.copy(userLocation = fix, locating = false, locationNotice = null) }
            }
        }
    }

    /**
     * Startup locate: begin streaming (no follow) so the map can recentre on the first
     * fix. Surfaces a [LocationNotice] when location services are off (the flow would
     * otherwise never emit) or when no fix arrives within [LOCATE_TIMEOUT_MS] (slow GPS) —
     * the map stays on the fallback camera, and a late fix still recentres it.
     */
    fun beginInitialLocate() {
        if (!location.hasPermission()) return
        if (!location.isLocationEnabled()) {
            _state.update { it.copy(locating = false, locationNotice = LocationNotice.ServicesOff) }
            return
        }
        enableLocation()
        // GPS can take a while for the first fix — don't surface an error if it's
        // simply slow. After the window, quietly drop the "locating…" hint; a late
        // fix still recentres. (Services-off, handled above, is the only notice.)
        viewModelScope.launch {
            kotlinx.coroutines.delay(LOCATE_TIMEOUT_MS)
            if (_state.value.userLocation == null) {
                _state.update { it.copy(locating = false) }
            }
        }
    }

    fun dismissLocationNotice() = _state.update { it.copy(locationNotice = null) }

    // Persist the choice; the settings collector above reflects it back into state.
    fun setBaseLayer(layer: BaseLayer) {
        viewModelScope.launch { settings.setBaseLayer(layer) }
    }
    fun setFollowing(value: Boolean) = _state.update { it.copy(following = value) }

    /** Persist the current map camera so the next launch reopens here. */
    fun saveCamera(lat: Double, lng: Double, zoom: Double) {
        viewModelScope.launch { settings.setLastCamera(lat, lng, zoom) }
    }

    /** Toggle 3D map mode (orbit gestures). No-op effect unless the wgpu engine is active. */
    fun setThreeDMode(value: Boolean) = _state.update { it.copy(threeDMode = value) }

    /** Resolve a human label for [point] (used to pre-fill a new marker's name). */
    fun describePoint(point: LatLng) {
        _pointDescription.value = null
        describeJob?.cancel()
        describeJob = viewModelScope.launch {
            _pointDescription.value = (reverseGeocode.describe(point) as? Outcome.Success)?.value
        }
    }

    /** Clear the pending point description (on sheet dismiss). */
    fun clearPointDescription() {
        describeJob?.cancel()
        _pointDescription.value = null
    }

    /** Persist a new user marker (offline-first; the map updates via [observeAll]). */
    fun addMarker(
        name: String,
        kind: ActivityKindId,
        position: LatLng,
        colorArgb: Long? = null,
        notes: String? = null,
    ) {
        val safeName = name.ifBlank { strings.get(kind.labelRes) }
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

    private companion object {
        /** How long to wait for the first fix before showing the slow-GPS notice. */
        const val LOCATE_TIMEOUT_MS = 12_000L
    }
}
