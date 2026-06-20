package com.sigmundgranaas.turbo.expressive.feature.map

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.Composable
import com.sigmundgranaas.turbo.expressive.core.map.MapSelectionState
import com.sigmundgranaas.turbo.expressive.domain.GeoBounds
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import com.sigmundgranaas.turbo.expressive.feature.map.live.LiveDetent
import com.sigmundgranaas.turbo.expressive.feature.photos.PhotoCluster
import com.sigmundgranaas.turbo.expressive.domain.MapEngine

/**
 * All of [MapScreen]'s transient UI state in one place — the map controller, the
 * live-sheet detents, the create-track tool's working geometry, and every dialog /
 * sheet trigger. Pulling these ~30 `remember`s out of the screen keeps the composable
 * focused on orchestration (effects + content), and lets the modal layer take this one
 * object instead of two dozen value+callback pairs. Pure state: VM calls and effects
 * stay in the composable. (`didInitialCenter` stays a `rememberSaveable` in the screen
 * so the one-shot recentre survives process death.)
 */
@Stable
class MapScreenState {
    // ── Camera / map handle ──
    var controller by mutableStateOf<MapEngine?>(null)
    var bearing by mutableFloatStateOf(0f)
    /** Bumped on every camera-idle so on-map chrome that reads the camera (e.g. the
     *  offline-coverage chip) recomposes after a pan/zoom, not just on rotation. */
    var cameraIdleTick by mutableIntStateOf(0)
    /** A saved track opened on the map ("Show on map") — drawn + selected. */
    var displayedTrack by mutableStateOf<List<LatLng>?>(null)

    // ── Selection + live-sheet detents ──
    val selectionState = MapSelectionState()
    var recDetent by mutableStateOf(LiveDetent.Half)
    var followDetent by mutableStateOf(LiveDetent.Half)
    var followServiceStarted by mutableStateOf(false)

    // ── Layers / overlays ──
    var showLayers by mutableStateOf(false)
    var activeOverlays by mutableStateOf<Set<OverlayId>>(emptySet())
    /** Procedural weather-cloud overlay on/off (wgpu engine only). Toggled from
     *  the Layers sheet; drives the bottom scrubber control. */
    var cloudsOn by mutableStateOf(false)
    /** Nasjonal Turbase (ut.no / DNT) "Cabins & trips" overlay on/off. Toggled
     *  from the Layers sheet; drives the in-view POI fetch + on-map pins. */
    var showCabins by mutableStateOf(false)
    /** The captured viewport awaiting the "download this area" size-confirm dialog. */
    var pendingDownloadArea by mutableStateOf<PendingDownloadArea?>(null)

    // ── Create-track tool (one tool, three modes); null = closed ──
    var trackMode by mutableStateOf<TrackMode?>(null)
    val linePoints = mutableStateListOf<LatLng>()
    val drawPoints = mutableStateListOf<LatLng>()
    var routeOrigin by mutableStateOf<LatLng?>(null)
    var selectedWaypoint by mutableStateOf<Int?>(null)
    var showRouteStyle by mutableStateOf(false)
    var showStops by mutableStateOf(false)
    var showTrackSave by mutableStateOf(false)
    var showTrackDiscard by mutableStateOf(false)

    // ── Journey save / follow prompts ──
    var showRouteSave by mutableStateOf(false)
    var showRecSave by mutableStateOf(false)
    var showFollowStopSave by mutableStateOf(false)
    var confirmReplaceFollow by mutableStateOf(false)
    /** "You moved while paused — include it?" prompt on resume (US-4) — recording. */
    var showResumeBufferPrompt by mutableStateOf(false)
    /** Same resume prompt for a paused **follow** (US-4, Follow = Record). */
    var showFollowResumeBufferPrompt by mutableStateOf(false)

    // ── Markers + on-map long-press ──
    var newMarkerAt by mutableStateOf<LatLng?>(null)
    var longPressAt by mutableStateOf<LatLng?>(null)
    /** Point whose full weather forecast sheet is open (from the long-press readout). */
    var forecastAt by mutableStateOf<LatLng?>(null)
    var editingMarker by mutableStateOf<Marker?>(null)
    var pendingDelete by mutableStateOf<Marker?>(null)
    var addToCollection by mutableStateOf<Marker?>(null)

    // ── Photos ──
    var openCluster by mutableStateOf<PhotoCluster?>(null)
    var viewerStart by mutableStateOf(-1)
    var pendingPhotoAt by mutableStateOf<LatLng?>(null)

    // Marker whose "Add photo" action was tapped → opens the camera/gallery chooser.
    var addPhotoForMarker by mutableStateOf<Marker?>(null)
}

/** The viewport captured when the user taps "Download this area", held until they
 *  confirm the size in [com.sigmundgranaas.turbo.expressive.feature.offline.DownloadAreaDialog]. */
data class PendingDownloadArea(val bounds: GeoBounds, val centre: LatLng, val zoom: Double)

@Composable
internal fun rememberMapScreenState(): MapScreenState = remember { MapScreenState() }
