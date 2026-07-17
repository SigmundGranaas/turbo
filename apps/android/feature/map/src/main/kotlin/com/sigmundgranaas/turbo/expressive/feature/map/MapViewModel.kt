package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.common.StringProvider
import com.sigmundgranaas.turbo.expressive.core.data.ConditionsRepository
import com.sigmundgranaas.turbo.expressive.core.tracking.LocationRepository
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
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.util.UUID
import javax.inject.Inject
import kotlin.math.roundToInt

/** Why an initial GPS centre couldn't happen — surfaced as a one-shot banner.
 *  Slow GPS is NOT a notice: the first fix just takes a while, so we wait
 *  quietly rather than nag. Only services-off (an actionable, persistent state)
 *  is surfaced. */
enum class LocationNotice { ServicesOff }

/**
 * Sea state pulled from the MET forecast, in the shape the water surface wants.
 * Any field may be null (MET drops marine data inland / wind at the tail).
 */
data class SeaState(
    val waveFromDeg: Float?,
    val waveHeightM: Float?,
    val windSpeedMs: Float?,
    val windFromDeg: Float?,
)

data class MapUiState(
    val markers: List<Marker> = emptyList(),
    val baseLayer: BaseLayer = BaseLayer.Norgeskart,
    val following: Boolean = false,
    val userLocation: LatLng? = null,
    /** Course over ground in degrees (0 = N), or null when the fix has no heading — drives the
     *  my-position heading beam on the map. */
    val userHeading: Float? = null,
    /** True while we've started locating but no fix has arrived yet (drives a "locating…" hint). */
    val locating: Boolean = false,
    val locationNotice: LocationNotice? = null,
    /** Persisted camera from the last session — restored on open so the user
     *  returns to where they left off. Null until the map has been moved once. */
    val lastCamera: LatLng? = null,
    val lastCameraZoom: Double? = null,
    /**
     * 3D terrain exaggeration `[0, MAX_3D_EXAGGERATION]` from the layers-sheet
     * slider. 0 = flat 2D (tilt locked); > 0 = 3D (orbit/tilt gestures unlocked +
     * terrain displaced by this exaggeration). Session state.
     */
    val threeDLevel: Float = 0f,
    /** Settings → Experimental gate: the Hiking-trails layer row only appears when on. */
    val experimentalTrails: Boolean = false,
    /** Settings → Experimental gate: the Weather-clouds layer row only appears when on. */
    val experimentalClouds: Boolean = false,
    /** True once the persisted settings (incl. the wgpu-map flag + last camera)
     *  have loaded at least once. The map host must wait for this so it's built
     *  ONCE with the right renderer + the restored camera — otherwise it builds
     *  with defaults (MapLibre, fallback camera), then swaps to the wgpu engine
     *  when the flag arrives, discarding the camera restore (every launch reset
     *  to the world overview). */
    val settingsLoaded: Boolean = false,
    /** My-position dot colour ("#RRGGBB"); null = default blue. Mirrors settings. */
    val locationDotColorHex: String? = null,
    /** Whether the my-position heading beam is drawn (settings toggle). */
    val showHeadingBeam: Boolean = true,
    /** User-added XYZ basemaps, and the active one (null = built-in [baseLayer]). */
    val customTileSources: List<com.sigmundgranaas.turbo.expressive.domain.CustomTileSource> = emptyList(),
    val selectedCustomSource: com.sigmundgranaas.turbo.expressive.domain.CustomTileSource? = null,
)

/** Holds the map home's UI state; markers + live location come from repositories. */
@HiltViewModel
class MapViewModel @Inject constructor(
    private val markerRepository: MarkerRepository,
    private val location: LocationRepository,
    private val reverseGeocode: ReverseGeocodeRepository,
    private val strings: StringProvider,
    private val settings: SettingsRepository,
    private val conditions: ConditionsRepository,
    radarRepository: RadarRepository,
) : ViewModel() {
    private val _state = MutableStateFlow(MapUiState())
    val state: StateFlow<MapUiState> = _state.asStateFlow()

    /**
     * Latest sea state (MET wave/wind forecast) near the user, feeding the wgpu
     * water surface — wave direction + ferocity, whitecaps, shoreline foam. Null
     * until a fix + forecast arrive (or inland, where MET has no marine data, the
     * wave fields stay null and the water falls back to a calm look).
     */
    private val _seaState = MutableStateFlow<SeaState?>(null)
    val seaState: StateFlow<SeaState?> = _seaState.asStateFlow()

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
        // Sea state for the water surface: follow the user's location (rounded to
        // ~0.02° ≈ 2 km so we don't hammer MET on every GPS sample), fetch the
        // MET wave/wind forecast, and publish it. `collectLatest` cancels an
        // in-flight fetch when the location moves on. Failures leave the last
        // good value (the water just keeps its previous look).
        viewModelScope.launch {
            location.locationUpdates()
                .map { LatLng((it.lat * 50.0).roundToInt() / 50.0, (it.lng * 50.0).roundToInt() / 50.0) }
                .distinctUntilChanged()
                .collectLatest { p ->
                    when (val outcome = conditions.forPoint(p)) {
                        is Outcome.Success -> {
                            val c = outcome.value
                            _seaState.value = SeaState(
                                waveFromDeg = c.marine?.waveFromDeg?.toFloat(),
                                waveHeightM = c.marine?.waveHeightM?.toFloat(),
                                windSpeedMs = c.weather?.windSpeedMs?.toFloat(),
                                windFromDeg = c.weather?.windFromDeg?.toFloat(),
                            )
                        }
                        is Outcome.Failure -> Unit
                    }
                }
        }
        // Restore (and keep in sync with) the persisted base map so the choice
        // survives relaunch instead of resetting to Norgeskart every time.
        viewModelScope.launch {
            settings.settings.collect { s ->
                _state.update {
                    it.copy(
                        baseLayer = s.baseLayer,
                        lastCamera = if (s.lastCameraLat != null && s.lastCameraLng != null) {
                            LatLng(s.lastCameraLat!!, s.lastCameraLng!!)
                        } else {
                            null
                        },
                        lastCameraZoom = s.lastCameraZoom,
                        settingsLoaded = true,
                        locationDotColorHex = s.locationDotColorHex,
                        showHeadingBeam = s.showHeadingBeam,
                        customTileSources = s.customTileSources,
                        selectedCustomSource = s.customTileSources.firstOrNull { c -> c.id == s.selectedCustomSourceId },
                        experimentalTrails = s.experimentalTrails,
                        experimentalClouds = s.experimentalClouds,
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
            location.samples().collect { s ->
                _state.update {
                    it.copy(
                        userLocation = s.position,
                        userHeading = s.bearingDeg?.toFloat(),
                        locating = false,
                        locationNotice = null,
                    )
                }
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
    fun addCustomTileSource(name: String, urlTemplate: String) {
        viewModelScope.launch {
            settings.addCustomTileSource(
                com.sigmundgranaas.turbo.expressive.domain.CustomTileSource(
                    id = java.util.UUID.randomUUID().toString(),
                    name = name.trim().ifBlank { "Custom map" },
                    urlTemplate = urlTemplate.trim(),
                ),
            )
        }
    }

    fun removeCustomTileSource(id: String) {
        viewModelScope.launch { settings.removeCustomTileSource(id) }
    }

    fun selectCustomTileSource(id: String?) {
        viewModelScope.launch { settings.selectCustomTileSource(id) }
    }

    fun setBaseLayer(layer: BaseLayer) {
        viewModelScope.launch { settings.setBaseLayer(layer) }
    }
    fun setFollowing(value: Boolean) = _state.update { it.copy(following = value) }

    /** Persist the current map camera so the next launch reopens here. */
    fun saveCamera(lat: Double, lng: Double, zoom: Double) {
        viewModelScope.launch { settings.setLastCamera(lat, lng, zoom) }
    }

    /** Set the 3D-terrain exaggeration (layers-sheet slider). 0 = flat 2D. */
    fun setThreeDLevel(value: Float) = _state.update { it.copy(threeDLevel = value) }

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
