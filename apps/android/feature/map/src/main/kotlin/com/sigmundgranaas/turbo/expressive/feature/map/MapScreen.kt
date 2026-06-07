package com.sigmundgranaas.turbo.expressive.feature.map

import android.Manifest
import android.os.Build
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DeleteSweep
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.Route
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.core.geo.formatCoords
import com.sigmundgranaas.turbo.expressive.core.map.MapEntityDetailHost
import com.sigmundgranaas.turbo.expressive.core.map.MapSelection
import com.sigmundgranaas.turbo.expressive.core.map.MapSelectionState
import com.sigmundgranaas.turbo.expressive.core.map.defaultMapEntityActionRegistry
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.SampleData
import com.sigmundgranaas.turbo.expressive.feature.conditions.ConditionsBody
import com.sigmundgranaas.turbo.expressive.feature.layers.MapLayersSheet
import com.sigmundgranaas.turbo.expressive.feature.markers.MarkerEditorSheet
import com.sigmundgranaas.turbo.expressive.feature.nav.DrawerDestination
import com.sigmundgranaas.turbo.expressive.feature.nav.NavDrawerContent
import com.sigmundgranaas.turbo.expressive.feature.offline.OfflineViewModel
import com.sigmundgranaas.turbo.expressive.feature.recording.RecordingViewModel
import com.sigmundgranaas.turbo.expressive.feature.recording.TrackSaveDialog
import com.sigmundgranaas.turbo.expressive.ui.components.DeleteMarkerDialog
import com.sigmundgranaas.turbo.expressive.ui.components.MapControlRail
import com.sigmundgranaas.turbo.expressive.ui.components.NameInputDialog
import com.sigmundgranaas.turbo.expressive.ui.components.rememberTurboHaptics
import com.sigmundgranaas.turbo.expressive.ui.components.SearchPill
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.map.MapController
import com.sigmundgranaas.turbo.expressive.ui.map.TurboMap
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import com.sigmundgranaas.turbo.expressive.ui.theme.icon
import com.sigmundgranaas.turbo.expressive.ui.theme.labelRes
import kotlinx.coroutines.launch

/** How far off the planned line (metres) before we re-solve while following. */
private const val OFF_ROUTE_THRESHOLD_M = 50.0

@Composable
fun MapScreen(
    onOpenSearch: () -> Unit,
    onOpenSettings: () -> Unit,
    onOpenPaths: () -> Unit,
    onOpenOffline: () -> Unit,
    onOpenCollections: () -> Unit = {},
    focusRequest: LatLng? = null,
    onFocusConsumed: () -> Unit = {},
    showTrackId: String? = null,
    onShowTrackConsumed: () -> Unit = {},
    viewModel: MapViewModel = hiltViewModel(),
    routeViewModel: RouteViewModel = hiltViewModel(),
    offlineViewModel: OfflineViewModel = hiltViewModel(),
    recordingViewModel: RecordingViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val context = androidx.compose.ui.platform.LocalContext.current
    val haptics = rememberTurboHaptics()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val routeState by routeViewModel.state.collectAsStateWithLifecycle()
    val routePreset by routeViewModel.preset.collectAsStateWithLifecycle()
    val recState by recordingViewModel.state.collectAsStateWithLifecycle()
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()

    var controller by remember { mutableStateOf<MapController?>(null) }
    var bearing by remember { mutableFloatStateOf(0f) }
    val metric = com.sigmundgranaas.turbo.expressive.ui.theme.LocalMetricUnits.current
    val selectionState = remember { MapSelectionState() }
    // A saved track opened on the map ("Show on map" from a track) — drawn + selected.
    var displayedTrack by remember { mutableStateOf<List<LatLng>?>(null) }

    // Open a saved track on the map: draw it, frame the camera, and select it so the
    // detail sheet (with Follow) appears. This makes saved tracks live on the map
    // instead of dead-ending in a list/sketch.
    LaunchedEffect(showTrackId, controller) {
        val id = showTrackId ?: return@LaunchedEffect
        val ctrl = controller ?: return@LaunchedEffect
        val path = routeViewModel.pathById(id) ?: run { onShowTrackConsumed(); return@LaunchedEffect }
        val pts = path.path.points
        if (pts.size < 2) { onShowTrackConsumed(); return@LaunchedEffect }
        displayedTrack = pts
        ctrl.frameTo(pts)
        val ascent = path.path.ascentM ?: 0.0
        selectionState.select(
            com.sigmundgranaas.turbo.expressive.core.map.MapSelection(
                id = "track-${path.id}",
                title = path.name,
                subtitle = "${com.sigmundgranaas.turbo.expressive.core.geo.Units.distance(path.path.distanceM, metric)} · ↑ ${com.sigmundgranaas.turbo.expressive.core.geo.Units.elevation(ascent, metric)}",
                icon = androidx.compose.material.icons.Icons.Rounded.Route,
                includeStandardActions = false,
                extraActions = listOf(
                    com.sigmundgranaas.turbo.expressive.core.map.MapEntityAction(
                        id = "follow_track",
                        label = context.getString(R.string.route_follow),
                        icon = androidx.compose.material.icons.Icons.Rounded.Navigation,
                        onInvoke = {
                            routeViewModel.followTrack(pts, path.path.distanceM, ascent, (path.path.movingTimeSeconds ?: 0).toDouble())
                            viewModel.enableLocation()
                            viewModel.setFollowing(true)
                        },
                    ),
                ),
            ),
        )
        onShowTrackConsumed()
    }

    // The opened track stays drawn only while its sheet is up.
    LaunchedEffect(selectionState.selection?.id) {
        if (selectionState.selection?.id?.startsWith("track-") != true) displayedTrack = null
    }

    val locationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        if (granted) {
            viewModel.enableLocation()
            viewModel.setFollowing(true)
        }
    }

    // ---- Recording (a mode of this map, not a separate screen) ----
    var showRecSave by remember { mutableStateOf(false) }
    val notificationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { /* best-effort: the ongoing notification only shows if granted */ }
    val recordingLocationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted -> recordingViewModel.onPermissionResult(granted) }
    // Start a recording from the map: clear the notification gate (Android 13+),
    // then start the foreground service if located — else ask and start on grant.
    fun startRecording() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        if (viewModel.hasLocationPermission()) {
            recordingViewModel.start()
        } else {
            recordingLocationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
        }
    }

    // Show the user's location on first load if the permission is already granted.
    LaunchedEffect(Unit) { if (viewModel.hasLocationPermission()) viewModel.enableLocation() }

    // While recording, keep the camera on the latest fix — recording implies movement,
    // even if the user never toggled "follow".
    LaunchedEffect(recState.userLocation, recState.recording) {
        if (recState.recording) recState.userLocation?.let { controller?.flyTo(it, 16.0) }
    }

    // While following, keep the camera centred on the latest fix.
    LaunchedEffect(state.userLocation, state.following) {
        if (state.following) state.userLocation?.let { controller?.flyTo(it, 15.0) }
    }

    // Off-route detection: re-solve from the current fix if the user strays from the route.
    LaunchedEffect(routeState, state.userLocation) {
        val following = routeState as? RouteUiState.Following ?: return@LaunchedEffect
        val here = state.userLocation ?: return@LaunchedEffect
        if (GeoMetrics.distanceToPath(following.plan.geometry, here) > OFF_ROUTE_THRESHOLD_M) {
            routeViewModel.reroute(here)
        }
    }

    // A search pick (or other external request) flies the camera to a coordinate
    // once the map controller is ready, then clears the one-shot request.
    LaunchedEffect(focusRequest, controller) {
        val target = focusRequest ?: return@LaunchedEffect
        controller?.let {
            it.flyTo(target, 14.0)
            onFocusConsumed()
        }
    }
    var showLayers by remember { mutableStateOf(false) }
    // Data overlay composited over the base map (null = none).
    var activeOverlays by remember { mutableStateOf<Set<com.sigmundgranaas.turbo.expressive.domain.OverlayId>>(emptySet()) }
    var showRouteSave by remember { mutableStateOf(false) }
    // ── Create track tool — one tool, three modes (Route/Line/Draw). null = closed.
    // It replaces the old standalone measuring tool and the route-planning card:
    // Route delegates to routeViewModel (snap to trails); Line/Draw build local geometry.
    var trackMode by remember { mutableStateOf<TrackMode?>(null) }
    val linePoints = remember { mutableStateListOf<LatLng>() }
    val drawPoints = remember { mutableStateListOf<LatLng>() }
    // First Route tap with no GPS fix becomes the origin; the second starts the solve.
    var routeOrigin by remember { mutableStateOf<LatLng?>(null) }
    var showRouteStyle by remember { mutableStateOf(false) }
    var showTrackSave by remember { mutableStateOf(false) }
    var showTrackDiscard by remember { mutableStateOf(false) }
    // Open the tool fresh in [mode], wiping any half-built geometry.
    val openTrackTool: (TrackMode) -> Unit = { mode ->
        routeViewModel.clear(); linePoints.clear(); drawPoints.clear(); routeOrigin = null
        viewModel.setFollowing(false)
        selectionState.clear()
        trackMode = mode
    }
    // Close the tool; keep the route only when we're handing off to Follow.
    val closeTrackTool: (Boolean) -> Unit = { keepRoute ->
        if (!keepRoute) routeViewModel.clear()
        linePoints.clear(); drawPoints.clear(); routeOrigin = null; trackMode = null
    }
    // Position long-pressed on the map → drives the "new marker" sheet (null = closed).
    var newMarkerAt by remember { mutableStateOf<LatLng?>(null) }
    // Marker being edited (null = no editor open).
    var editingMarker by remember { mutableStateOf<Marker?>(null) }
    // Marker pending delete-confirmation (null = no dialog).
    var pendingDelete by remember { mutableStateOf<Marker?>(null) }
    // Marker whose "add to collection" picker is open (null = closed).
    var addToCollection by remember { mutableStateOf<Marker?>(null) }

    // One selection model + detail host — the map shell no longer depends on the
    // markers feature for the info sheet; it routes through the :core:map seam.
    val actionRegistry = remember { defaultMapEntityActionRegistry() }

    ModalNavigationDrawer(
        drawerState = drawerState,
        // Only intercept swipes while open (swipe-to-close); otherwise the map owns
        // all pan gestures so a left-edge drag pans instead of opening the drawer.
        gesturesEnabled = drawerState.isOpen,
        drawerContent = {
            NavDrawerContent(selected = DrawerDestination.Map) { dest ->
                scope.launch { drawerState.close() }
                when (dest) {
                    DrawerDestination.Settings -> onOpenSettings()
                    DrawerDestination.Paths -> onOpenPaths()
                    DrawerDestination.Collections -> onOpenCollections()
                    DrawerDestination.Record -> startRecording()
                    DrawerDestination.Offline -> onOpenOffline()
                    DrawerDestination.Map -> Unit
                }
            }
        },
    ) {
        Box(Modifier.fillMaxSize()) {
            TurboMap(
                base = state.baseLayer,
                overlays = activeOverlays,
                initialCamera = SampleData.initialCamera,
                initialZoom = SampleData.initialZoom,
                markers = state.markers,
                route = routeState.polyline.takeIf { it.isNotEmpty() },
                // The track overlay shows, in priority: the live recording trail, the
                // Line/Draw geometry being built in the Create track tool, else whatever
                // saved track the user opened ("Show on map").
                track = when {
                    recState.recording -> recState.points.takeIf { it.size > 1 }
                    trackMode == TrackMode.Line -> linePoints.takeIf { it.size > 1 }?.toList()
                    trackMode == TrackMode.Draw -> drawPoints.takeIf { it.size > 1 }?.toList()
                    else -> displayedTrack
                },
                selectedMarkerId = selectionState.selection?.id,
                userLocation = state.userLocation,
                onMarkerClick = { marker ->
                    selectionState.select(
                        MapSelection(
                            id = marker.id,
                            title = marker.name,
                            subtitle = "${context.getString(marker.kind.labelRes)} · ${formatCoords(marker.position)}",
                            icon = marker.kind.icon,
                            point = marker.position,
                            onNavigate = {
                                // Navigate-to-marker opens the unified Create track tool in
                                // Route mode. If a route is already being built, drop this
                                // marker in as a stop instead of starting over.
                                if (trackMode == TrackMode.Route && routeViewModel.waypoints.value.size >= 2) {
                                    routeViewModel.addStop(marker.position)
                                } else {
                                    val from = state.userLocation ?: controller?.center() ?: SampleData.initialCamera
                                    openTrackTool(TrackMode.Route)
                                    routeViewModel.planRoute(from, marker.position)
                                }
                            },
                            onShare = { shareMarkerGeoJson(context, marker) },
                            onEdit = { editingMarker = marker },
                            onDelete = { pendingDelete = marker },
                            extraActions = listOf(
                                com.sigmundgranaas.turbo.expressive.core.map.MapEntityAction(
                                    id = "add_to_collection",
                                    label = context.getString(R.string.marker_add_to_collection),
                                    icon = androidx.compose.material.icons.Icons.Rounded.Folder,
                                    onInvoke = { addToCollection = marker },
                                ),
                            ),
                            body = {
                                Column {
                                    if (!marker.notes.isNullOrBlank()) {
                                        Text(
                                            marker.notes!!,
                                            style = MaterialTheme.typography.bodyMedium,
                                            color = cs.onSurface,
                                        )
                                        Spacer(Modifier.height(14.dp))
                                    }
                                    com.sigmundgranaas.turbo.expressive.feature.photos.MarkerPhotos(
                                        markerId = marker.id,
                                        position = marker.position,
                                    )
                                    Spacer(Modifier.height(14.dp))
                                    ConditionsBody(marker.position)
                                }
                            },
                        ),
                    )
                },
                // Dot overlay marks Line vertices, and the pending Route start (the first
                // tap before a destination exists) so it doesn't look like nothing happened.
                measurePoints = when (trackMode) {
                    TrackMode.Line -> linePoints
                    TrackMode.Route -> listOfNotNull(routeOrigin)
                    else -> emptyList()
                },
                onMapLongClick = { if (trackMode == null) { haptics.longPress(); newMarkerAt = it } },
                onMapTap = { p ->
                    when (trackMode) {
                        TrackMode.Route -> {
                            haptics.toggle(true)
                            if (routeViewModel.waypoints.value.size >= 2) {
                                routeViewModel.addStop(p)
                            } else {
                                val origin = state.userLocation ?: routeOrigin
                                if (origin == null) routeOrigin = p
                                else { routeViewModel.planRoute(origin, p); routeOrigin = null }
                            }
                        }
                        TrackMode.Line -> { haptics.toggle(true); linePoints.add(p) }
                        TrackMode.Draw -> Unit // handled by the drag overlay
                        null -> selectionState.clear()
                    }
                },
                onMapReady = { controller = it },
                onBearingChange = { bearing = it.toFloat() },
                modifier = Modifier.fillMaxSize(),
            )

            // Freehand Draw capture: a transparent layer that turns finger drags into
            // track points (and consumes the gesture so the map doesn't pan).
            if (trackMode == TrackMode.Draw) {
                Box(
                    Modifier.fillMaxSize().pointerInput(Unit) {
                        detectDragGestures(
                            onDragStart = { off ->
                                drawPoints.clear()
                                controller?.fromScreen(off.x, off.y)?.let { drawPoints.add(it) }
                            },
                            onDrag = { change, _ ->
                                controller?.fromScreen(change.position.x, change.position.y)?.let { drawPoints.add(it) }
                            },
                        )
                    },
                )
            }

            // Chrome is hidden while the Create track tool owns the screen.
            if (trackMode == null) {
                SearchPill(
                    placeholder = "Search places, coordinates…",
                    onMenuClick = { scope.launch { drawerState.open() } },
                    onClick = onOpenSearch,
                    modifier = Modifier
                        .align(Alignment.TopCenter)
                        .windowInsetsPadding(WindowInsets.statusBars)
                        .padding(16.dp),
                )

                MapControlRail(
                    following = state.following,
                    creatingTrack = false,
                    bearing = bearing,
                    onCompass = { controller?.resetNorth() },
                    onAdd = { (controller?.center() ?: state.userLocation)?.let { newMarkerAt = it } },
                    onCreateTrack = { openTrackTool(TrackMode.Route) },
                    onLayers = { showLayers = true },
                    onLocate = {
                        if (viewModel.hasLocationPermission()) {
                            viewModel.enableLocation()
                            val next = !state.following
                            viewModel.setFollowing(next)
                            if (next) state.userLocation?.let { controller?.flyTo(it, 15.0) }
                        } else {
                            locationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
                        }
                    },
                    onZoomIn = { controller?.zoomIn() },
                    onZoomOut = { controller?.zoomOut() },
                    modifier = Modifier
                        .align(Alignment.CenterEnd)
                        .windowInsetsPadding(WindowInsets.statusBars)
                        .padding(end = 14.dp),
                )
            }

            if (state.following) {
                Surface(
                    shape = CircleShape, color = cs.tertiaryContainer, shadowElevation = 2.dp,
                    modifier = Modifier.align(Alignment.BottomStart)
                        .windowInsetsPadding(WindowInsets.navigationBars).padding(start = 16.dp, bottom = 36.dp),
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 16.dp, vertical = 9.dp)) {
                        Icon(Icons.Rounded.MyLocation, null, tint = cs.onTertiaryContainer, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(8.dp))
                        Text("Following", style = MaterialTheme.typography.labelLarge, color = cs.onTertiaryContainer)
                    }
                }
            }

            Text(
                "© Kartverket",
                style = MaterialTheme.typography.labelSmall,
                color = cs.onSurfaceVariant,
                modifier = Modifier.align(Alignment.BottomStart)
                    .windowInsetsPadding(WindowInsets.navigationBars)
                    .background(cs.surface.copy(alpha = 0.7f))
                    .padding(horizontal = 7.dp, vertical = 2.dp),
            )

            // ── Bottom journey surface ──
            // Recording controls win the slot; otherwise — when the Create track tool
            // is closed — the route card shows live following. Route *planning* now
            // lives inside the tool, so RouteCard here is the Following/Error surface.
            if (recState.recording) {
                RecordingControls(
                    journey = ActiveJourney(
                        mode = JourneyMode.Recording,
                        geometry = recState.points,
                        distanceM = recState.distanceM,
                        elapsedSec = recState.elapsedSec,
                        paused = recState.paused,
                    ),
                    metric = metric,
                    onPause = { haptics.toggle(recState.paused); recordingViewModel.togglePause() },
                    onStop = { haptics.confirm(); recordingViewModel.stop(); showRecSave = true },
                    modifier = Modifier.align(Alignment.BottomCenter)
                        .windowInsetsPadding(WindowInsets.navigationBars)
                        .padding(16.dp),
                )
            } else if (trackMode == null) {
                val routeWaypoints by routeViewModel.waypoints.collectAsStateWithLifecycle()
                RouteCard(
                    state = routeState,
                    preset = routePreset,
                    userLocation = state.userLocation,
                    waypointCount = routeWaypoints.size,
                    onRemoveStop = { index -> routeViewModel.removeWaypoint(index) },
                    onSelectPreset = { routeViewModel.selectPreset(it) },
                    onFollow = {
                        if (viewModel.hasLocationPermission()) {
                            viewModel.enableLocation()
                            viewModel.setFollowing(true)
                            routeViewModel.follow()
                        } else {
                            locationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
                        }
                    },
                    onSave = { showRouteSave = true },
                    onDownloadOffline = { routeViewModel.downloadAlongRoute(state.baseLayer) },
                    onClear = { routeViewModel.clear(); viewModel.setFollowing(false) },
                    conditions = {
                        val line = routeState.polyline
                        if (line.size > 1) {
                            com.sigmundgranaas.turbo.expressive.feature.conditions.RouteConditionsStrip(line)
                        }
                    },
                    modifier = Modifier.align(Alignment.BottomCenter)
                        .windowInsetsPadding(WindowInsets.navigationBars)
                        .padding(16.dp),
                )
            }

            // ── Create track tool: chrome (close + title) + the unified panel ──
            trackMode?.let { mode ->
                val donePlan = (routeState as? RouteUiState.Done)?.plan
                val geometry = when (mode) {
                    TrackMode.Route -> routeState.polyline
                    TrackMode.Line -> linePoints.toList()
                    TrackMode.Draw -> drawPoints.toList()
                }
                val distM = when (mode) {
                    TrackMode.Route -> donePlan?.distanceM ?: 0.0
                    else -> GeoMetrics.pathLengthMeters(geometry)
                }
                val full = com.sigmundgranaas.turbo.expressive.core.geo.Units.distance(distM, metric)
                val distText = full.substringBeforeLast(' ')
                val unitText = if (full.contains(' ')) full.substringAfterLast(' ') else ""
                val meta = when (mode) {
                    TrackMode.Route -> when (val s = routeState) {
                        is RouteUiState.Solving -> stringResource(R.string.route_solving)
                        is RouteUiState.Done -> "${formatDuration(s.plan.durationS)}  ·  ↑ ${com.sigmundgranaas.turbo.expressive.core.geo.Units.elevation(s.plan.ascentM, metric)}"
                        is RouteUiState.Error -> s.message
                        else -> stringResource(R.string.track_meta_empty)
                    }
                    TrackMode.Line -> if (linePoints.isNotEmpty()) stringResource(R.string.track_meta_line, linePoints.size) else stringResource(R.string.track_meta_empty)
                    TrackMode.Draw -> if (drawPoints.size >= 2) stringResource(R.string.track_meta_draw) else stringResource(R.string.track_meta_empty)
                }
                val canSave = if (mode == TrackMode.Route) routeState is RouteUiState.Done else geometry.size >= 2
                val canUndo = when (mode) {
                    TrackMode.Route -> routeViewModel.canUndo
                    TrackMode.Line -> linePoints.isNotEmpty()
                    TrackMode.Draw -> drawPoints.isNotEmpty()
                }
                // Anything the user has placed that a close would throw away.
                val hasUnsaved = when (mode) {
                    TrackMode.Route -> routeViewModel.waypoints.value.isNotEmpty() || routeOrigin != null
                    TrackMode.Line -> linePoints.isNotEmpty()
                    TrackMode.Draw -> drawPoints.isNotEmpty()
                }

                CreateTrackCloseButton(
                    onClose = { if (hasUnsaved) showTrackDiscard = true else closeTrackTool(false) },
                    modifier = Modifier.align(Alignment.TopStart)
                        .windowInsetsPadding(WindowInsets.statusBars).padding(16.dp),
                )
                CreateTrackTitleChip(
                    modifier = Modifier.align(Alignment.TopCenter)
                        .windowInsetsPadding(WindowInsets.statusBars).padding(top = 18.dp),
                )
                // Keep zoom + recenter reachable — the full rail is hidden in the tool.
                CreateTrackMapControls(
                    following = state.following,
                    onLocate = {
                        if (viewModel.hasLocationPermission()) {
                            viewModel.enableLocation()
                            state.userLocation?.let { controller?.flyTo(it, 15.0) }
                        } else {
                            locationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
                        }
                    },
                    onZoomIn = { controller?.zoomIn() },
                    onZoomOut = { controller?.zoomOut() },
                    modifier = Modifier.align(Alignment.CenterEnd)
                        .windowInsetsPadding(WindowInsets.statusBars).padding(end = 14.dp),
                )
                Column(
                    Modifier.align(Alignment.BottomCenter).fillMaxWidth()
                        .windowInsetsPadding(WindowInsets.navigationBars).padding(16.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    CreateTrackHint(mode = mode)
                    Spacer(Modifier.height(12.dp))
                    CreateTrackPanel(
                        mode = mode,
                        onMode = { next -> haptics.toggle(true); trackMode = next },
                        distanceText = distText,
                        unit = unitText,
                        metaText = meta,
                        surfaces = donePlan?.surfaces ?: emptyMap(),
                        presetLabel = routePreset.label,
                        onRouteStyle = { showRouteStyle = true },
                        canUndo = canUndo,
                        canSave = canSave,
                        onUndo = {
                            when (mode) {
                                TrackMode.Route -> routeViewModel.undo()
                                TrackMode.Line -> if (linePoints.isNotEmpty()) linePoints.removeAt(linePoints.lastIndex)
                                TrackMode.Draw -> drawPoints.clear()
                            }
                        },
                        onClear = {
                            when (mode) {
                                TrackMode.Route -> { routeViewModel.clear(); routeOrigin = null }
                                TrackMode.Line -> linePoints.clear()
                                TrackMode.Draw -> drawPoints.clear()
                            }
                        },
                        onSave = { haptics.toggle(true); showTrackSave = true },
                        onFollow = {
                            haptics.confirm()
                            if (mode == TrackMode.Route) routeViewModel.follow()
                            else routeViewModel.followTrack(geometry, distM, 0.0, 0.0)
                            if (viewModel.hasLocationPermission()) {
                                viewModel.enableLocation(); viewModel.setFollowing(true)
                            } else {
                                locationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
                            }
                            closeTrackTool(true)
                        },
                    )
                }

                if (showRouteStyle) {
                    RouteStyleSheet(
                        selected = routePreset,
                        onSelect = { routeViewModel.selectPreset(it); showRouteStyle = false },
                        onDismiss = { showRouteStyle = false },
                    )
                }
                if (showTrackSave) {
                    NameInputDialog(
                        title = stringResource(R.string.track_save_title),
                        confirmLabel = stringResource(com.sigmundgranaas.turbo.expressive.core.designsystem.R.string.ds_save),
                        initial = if (full.isNotBlank()) "Track $full" else "Track",
                        onConfirm = { name ->
                            if (mode == TrackMode.Route) routeViewModel.saveAsTrack(name)
                            else routeViewModel.saveLine(name, geometry)
                            Toast.makeText(context, R.string.route_saved, Toast.LENGTH_SHORT).show()
                            showTrackSave = false
                            closeTrackTool(false)
                        },
                        onDismiss = { showTrackSave = false },
                    )
                }
                if (showTrackDiscard) {
                    com.sigmundgranaas.turbo.expressive.ui.components.TurboConfirmDialog(
                        title = stringResource(R.string.track_discard_title),
                        body = stringResource(R.string.track_discard_body),
                        confirmLabel = stringResource(R.string.track_discard),
                        icon = androidx.compose.material.icons.Icons.Rounded.DeleteSweep,
                        destructive = true,
                        onConfirm = { showTrackDiscard = false; closeTrackTool(false) },
                        onDismiss = { showTrackDiscard = false },
                    )
                }
            }
        }
    }

    // ---- Selection detail host (markers, and any future entity) ----
    MapEntityDetailHost(state = selectionState, registry = actionRegistry)

    // ---- Tool sheets ----
    if (showLayers) {
        MapLayersSheet(
            selected = state.baseLayer,
            onSelectBase = viewModel::setBaseLayer,
            onDownloadArea = {
                controller?.let { ctrl ->
                    val bounds = ctrl.visibleBounds()
                    val centre = LatLng((bounds.north + bounds.south) / 2, (bounds.east + bounds.west) / 2)
                    offlineViewModel.download(formatCoords(centre), state.baseLayer, bounds, ctrl.zoom())
                }
                showLayers = false
                onOpenOffline()
            },
            activeOverlays = activeOverlays,
            onToggleOverlay = { id, on ->
                activeOverlays = if (on) activeOverlays + id else activeOverlays - id
            },
            onDismiss = { showLayers = false },
        )
    }
    // Add-to-collection picker for the selected marker.
    addToCollection?.let { marker ->
        com.sigmundgranaas.turbo.expressive.feature.collectionpicker.CollectionPickerSheet(
            itemId = marker.id,
            type = com.sigmundgranaas.turbo.expressive.domain.CollectionItemType.Marker,
            onDismiss = { addToCollection = null },
        )
    }
    // New marker: opened by a long-press on the map, anchored at that coordinate.
    // The coordinate is reverse-geocoded to pre-fill a sensible name ("Galdhøpiggen").
    newMarkerAt?.let { pos ->
        val description by viewModel.pointDescription.collectAsStateWithLifecycle()
        LaunchedEffect(pos) { viewModel.describePoint(pos) }
        MarkerEditorSheet(
            position = pos,
            suggestedName = description?.title,
            suggestedSubtitle = description?.let { listOfNotNull(it.label.takeIf { l -> l != it.title }, it.subtitle.takeIf(String::isNotBlank)).joinToString(" · ") },
            onDismiss = { newMarkerAt = null; viewModel.clearPointDescription() },
            onSave = { name, kind, color, notes ->
                viewModel.addMarker(name, kind, pos, color, notes)
                newMarkerAt = null
                viewModel.clearPointDescription()
            },
        )
    }
    // Edit marker: opened from the detail sheet's Edit action.
    editingMarker?.let { marker ->
        MarkerEditorSheet(
            position = marker.position,
            existing = marker,
            onDismiss = { editingMarker = null },
            onSave = { name, kind, color, notes ->
                viewModel.updateMarker(
                    marker.copy(name = name.ifBlank { marker.name }, kind = kind, colorArgb = color, notes = notes),
                )
                selectionState.clear()
                editingMarker = null
            },
        )
    }
    // Delete confirmation for the selected marker.
    pendingDelete?.let { marker ->
        DeleteMarkerDialog(
            markerName = marker.name,
            onConfirm = {
                viewModel.deleteMarker(marker.id)
                selectionState.clear()
                pendingDelete = null
            },
            onDismiss = { pendingDelete = null },
        )
    }
    // Name + save the planned route as a track (mirrors the recording save dialog).
    if (showRouteSave) {
        NameInputDialog(
            title = stringResource(R.string.route_save_title),
            confirmLabel = stringResource(com.sigmundgranaas.turbo.expressive.core.designsystem.R.string.ds_save),
            initial = "Route",
            onConfirm = { name ->
                routeViewModel.saveAsTrack(name)
                Toast.makeText(context, R.string.route_saved, Toast.LENGTH_SHORT).show()
                showRouteSave = false
            },
            onDismiss = { showRouteSave = false },
        )
    }
    // Finish a recording: name + activity-kind picker, then persist (or discard).
    if (showRecSave) {
        TrackSaveDialog(
            defaultName = "Track ${com.sigmundgranaas.turbo.expressive.core.geo.Units.distance(recState.distanceM, metric)}",
            canSave = recState.points.size > 1,
            onSave = { name, kind -> recordingViewModel.save(name, kind) {}; showRecSave = false },
            onDiscard = { recordingViewModel.discard {}; showRecSave = false },
            onDismiss = { showRecSave = false },
        )
    }
}


/** Export a single marker as a .geojson file and fire a share chooser. */
private fun shareMarkerGeoJson(context: android.content.Context, marker: Marker) {
    val dir = java.io.File(context.cacheDir, "markers").apply { mkdirs() }
    val file = java.io.File(dir, com.sigmundgranaas.turbo.expressive.feature.markers.MarkerGeoJson.fileName(marker.name))
    file.writeText(com.sigmundgranaas.turbo.expressive.feature.markers.MarkerGeoJson.encode(listOf(marker)))
    val uri = androidx.core.content.FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
    val send = android.content.Intent(android.content.Intent.ACTION_SEND).apply {
        type = "application/geo+json"
        putExtra(android.content.Intent.EXTRA_STREAM, uri)
        clipData = android.content.ClipData.newRawUri(marker.name, uri)
        addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    context.startActivity(android.content.Intent.createChooser(send, "Share marker").addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK))
}
