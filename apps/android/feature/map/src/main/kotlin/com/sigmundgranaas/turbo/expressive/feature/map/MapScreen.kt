package com.sigmundgranaas.turbo.expressive.feature.map

import android.Manifest
import android.os.Build
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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CloudOff
import androidx.compose.material.icons.rounded.DeleteSweep
import androidx.compose.material.icons.rounded.AddAPhoto
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.Navigation
import androidx.compose.material.icons.rounded.Route
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.Icon
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.core.geo.GeoMetrics
import com.sigmundgranaas.turbo.expressive.core.geo.formatCoords
import com.sigmundgranaas.turbo.expressive.core.map.MapSelection
import com.sigmundgranaas.turbo.expressive.core.map.defaultMapEntityActionRegistry
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.MapDefaults
import com.sigmundgranaas.turbo.expressive.feature.conditions.ConditionsBody
import com.sigmundgranaas.turbo.expressive.feature.nav.DrawerDestination
import com.sigmundgranaas.turbo.expressive.feature.nav.NavDrawerContent
import com.sigmundgranaas.turbo.expressive.feature.offline.OfflineViewModel
import com.sigmundgranaas.turbo.expressive.feature.recording.RecordingViewModel
import com.sigmundgranaas.turbo.expressive.ui.components.MapControlRail
import com.sigmundgranaas.turbo.expressive.ui.components.NameInputDialog
import com.sigmundgranaas.turbo.expressive.ui.components.rememberTurboHaptics
import com.sigmundgranaas.turbo.expressive.ui.components.SearchPill
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.layout.responsiveContentWidth
import com.sigmundgranaas.turbo.expressive.core.turbomap.android.TurbomapMapView
import com.sigmundgranaas.turbo.expressive.feature.map.radar.RadarOverlayControls
import com.sigmundgranaas.turbo.expressive.feature.map.sun.SunOverlayControls
import com.sigmundgranaas.turbo.expressive.ui.map.MapStyles
import com.sigmundgranaas.turbo.expressive.ui.map.TurboMap
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import com.sigmundgranaas.turbo.expressive.ui.theme.icon
import com.sigmundgranaas.turbo.expressive.ui.theme.labelRes
import kotlinx.coroutines.launch

/** How far off the planned line (metres) before we re-solve while following. */
private const val OFF_ROUTE_THRESHOLD_M = 50.0

/** Zoom used when first recentring on the user's GPS fix at startup. */
private const val INITIAL_LOCATION_ZOOM = 13.0

@Composable
fun MapScreen(
    onOpenSearch: () -> Unit,
    onOpenSettings: () -> Unit,
    onOpenPaths: () -> Unit,
    onOpenOffline: () -> Unit,
    onOpenCollections: () -> Unit = {},
    onOpenAccount: () -> Unit = {},
    accountEmail: String? = null,
    focusRequest: LatLng? = null,
    onFocusConsumed: () -> Unit = {},
    showTrackId: String? = null,
    onShowTrackConsumed: () -> Unit = {},
    // Set when launched from the Quick Settings tile — begin recording once foregrounded.
    autoStartRecording: Boolean = false,
    onAutoStartConsumed: () -> Unit = {},
    viewModel: MapViewModel = hiltViewModel(),
    routeViewModel: RouteViewModel = hiltViewModel(),
    offlineViewModel: OfflineViewModel = hiltViewModel(),
    recordingViewModel: RecordingViewModel = hiltViewModel(),
    offlineIndicator: com.sigmundgranaas.turbo.expressive.feature.offline.OfflineIndicatorViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val context = androidx.compose.ui.platform.LocalContext.current
    val haptics = rememberTurboHaptics()
    // All transient UI/tool/dialog state lives in one holder (see MapScreenState).
    val ui = rememberMapScreenState()
    val state by viewModel.state.collectAsStateWithLifecycle()
    val isOffline by offlineIndicator.offline.collectAsStateWithLifecycle()
    val routeState by routeViewModel.state.collectAsStateWithLifecycle()
    val routePreset by routeViewModel.preset.collectAsStateWithLifecycle()
    val toolWaypoints by routeViewModel.waypoints.collectAsStateWithLifecycle()
    // Which on-map route stop is selected (so dragging it moves it). Reset with the tool.
    val recState by recordingViewModel.state.collectAsStateWithLifecycle()
    val recSession by recordingViewModel.session.collectAsStateWithLifecycle()
    val followSession by routeViewModel.followSession.collectAsStateWithLifecycle()
    val seaState by viewModel.seaState.collectAsStateWithLifecycle()

    // Feed the live MET wave/wind forecast into the wgpu water surface (wave
    // direction + ferocity, whitecaps, shoreline foam). No-op on MapLibre, which
    // doesn't implement the capability.
    LaunchedEffect(ui.controller, seaState) {
        val s = seaState
        (ui.controller as? com.sigmundgranaas.turbo.expressive.domain.WaterConditionsOverlay)
            ?.setWaterConditions(s?.waveFromDeg, s?.waveHeightM, s?.windSpeedMs, s?.windFromDeg)
    }
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()

    val metric = com.sigmundgranaas.turbo.expressive.ui.theme.LocalMetricUnits.current
    // A saved track opened on the map ("Show on map" from a track) — drawn + selected.

    // Open a saved track on the map: draw it, frame the camera, and select it so the
    // detail sheet (with Follow) appears. This makes saved tracks live on the map
    // instead of dead-ending in a list/sketch.
    LaunchedEffect(showTrackId, ui.controller) {
        val id = showTrackId ?: return@LaunchedEffect
        val ctrl = ui.controller ?: return@LaunchedEffect
        val path = routeViewModel.pathById(id) ?: run { onShowTrackConsumed(); return@LaunchedEffect }
        val pts = path.path.points
        if (pts.size < 2) { onShowTrackConsumed(); return@LaunchedEffect }
        ui.displayedTrack = pts
        ctrl.frameTo(pts)
        val ascent = path.path.ascentM ?: 0.0
        ui.selectionState.select(
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
    LaunchedEffect(ui.selectionState.selection?.id) {
        if (ui.selectionState.selection?.id?.startsWith("track-") != true) ui.displayedTrack = null
    }

    val locationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        if (granted) {
            viewModel.enableLocation()
            viewModel.setFollowing(true)
        }
    }

    // Startup location: just begin streaming (no follow) so the map can centre on the
    // first real GPS fix once. Distinct from [locationPermission], which also locks follow.
    val startLocationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted -> if (granted) viewModel.beginInitialLocate() }

    // ---- Recording (a mode of this map, not a separate screen) ----
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

    // Quick Settings tile → "start recording": once the map is foregrounded (so the
    // location permission is accessible), begin recording, unless already running.
    LaunchedEffect(autoStartRecording) {
        if (autoStartRecording) {
            if (!recState.recording) startRecording()
            onAutoStartConsumed()
        }
    }

    // On first load, start locating (so the dot appears) — ask for the permission if
    // missing. We do NOT auto-follow: the app restores the camera the user last left
    // at (below), and follow is a manual toggle on the locate button.
    LaunchedEffect(Unit) {
        if (viewModel.hasLocationPermission()) {
            viewModel.beginInitialLocate()
        } else {
            startLocationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
        }
    }

    // One-shot startup camera: restore where the user last left off; only on a
    // first-ever launch (no saved camera) fall back to flying to the first GPS fix.
    // Either way it fires once and never fights manual panning afterwards.
    var didInitialCenter by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(state.lastCamera, ui.controller) {
        if (didInitialCenter || focusRequest != null) return@LaunchedEffect
        val cam = state.lastCamera ?: return@LaunchedEffect
        val c = ui.controller ?: return@LaunchedEffect
        c.flyTo(cam, state.lastCameraZoom ?: INITIAL_LOCATION_ZOOM)
        didInitialCenter = true
    }
    LaunchedEffect(state.userLocation, ui.controller) {
        if (didInitialCenter || focusRequest != null || state.lastCamera != null) return@LaunchedEffect
        val here = state.userLocation ?: return@LaunchedEffect
        val c = ui.controller ?: return@LaunchedEffect
        if (!state.following && !recState.recording) {
            c.flyTo(here, INITIAL_LOCATION_ZOOM)
            didInitialCenter = true
        }
    }

    // Persist the camera when the app goes to the background, so the next launch
    // reopens exactly where the user left off (restored by the effect above).
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_PAUSE) {
                ui.controller?.let { viewModel.saveCamera(it.center().lat, it.center().lng, it.zoom()) }
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    // While recording, keep the camera on the latest fix — recording implies movement,
    // even if the user never toggled "follow".
    LaunchedEffect(recState.userLocation, recState.recording) {
        if (recState.recording) recState.userLocation?.let { ui.controller?.flyTo(it, 16.0) }
    }

    // While following, keep the camera centred on the latest fix.
    LaunchedEffect(state.userLocation, state.following) {
        if (state.following) state.userLocation?.let { ui.controller?.flyTo(it, 15.0) }
    }

    // Mirror the follow session onto a foreground Live Update (lock-screen widget):
    // start it when following begins, dismiss it only after we actually started one
    // (so the initial inactive state doesn't tear down an unrelated recording service).
    LaunchedEffect(followSession.active) {
        if (followSession.active) {
            com.sigmundgranaas.turbo.expressive.feature.recording.RecordingService.startFollowing(context)
            ui.followServiceStarted = true
        } else if (ui.followServiceStarted) {
            com.sigmundgranaas.turbo.expressive.feature.recording.RecordingService.stopFollowing(context)
            ui.followServiceStarted = false
        }
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
    // once the map ui.controller is ready, then clears the one-shot request.
    LaunchedEffect(focusRequest, ui.controller) {
        val target = focusRequest ?: return@LaunchedEffect
        ui.controller?.let {
            it.flyTo(target, 14.0)
            onFocusConsumed()
        }
    }
    // Data overlay composited over the base map (null = none).
    // ── Create track tool — one tool, three modes (Route/Line/Draw). null = closed.
    // It replaces the old standalone measuring tool and the route-planning card:
    // Route delegates to routeViewModel (snap to trails); Line/Draw build local geometry.
    // First Route tap with no GPS fix becomes the origin; the second starts the solve.

    // Keep the user dot in the visible band above the live sheet: reserve bottom map
    // padding equal to the sheet's current detent height (capped at half the screen so
    // a full sheet doesn't shove the target off the top). Reset to 0 when no sheet shows.
    val density = LocalDensity.current
    val configuration = LocalConfiguration.current
    val liveDetent = if (recState.recording) ui.recDetent else ui.followDetent
    LaunchedEffect(ui.controller, recState.recording, routeState, ui.trackMode, liveDetent) {
        val c = ui.controller ?: return@LaunchedEffect
        val followingNow = routeState is RouteUiState.Following && ui.trackMode == null
        if (!recState.recording && !followingNow) { c.setBottomInset(0); return@LaunchedEffect }
        val screenPx = with(density) { configuration.screenHeightDp.dp.toPx() }
        val sheetPx = when (liveDetent) {
            com.sigmundgranaas.turbo.expressive.feature.map.live.LiveDetent.Mini ->
                with(density) { 208.dp.toPx() }.coerceAtMost(screenPx * 0.64f)
            com.sigmundgranaas.turbo.expressive.feature.map.live.LiveDetent.Peek ->
                with(density) { 340.dp.toPx() }.coerceAtMost(screenPx * 0.64f)
            com.sigmundgranaas.turbo.expressive.feature.map.live.LiveDetent.Half -> screenPx * 0.56f
            com.sigmundgranaas.turbo.expressive.feature.map.live.LiveDetent.Full -> screenPx * 0.92f
        }
        c.setBottomInset(sheetPx.coerceAtMost(screenPx * 0.5f).toInt())
    }
    // Stop-following save prompt + the snackbar that bridges Save → Follow.
    val snackbarHostState = remember { SnackbarHostState() }
    val savedToast = stringResource(R.string.route_saved)
    val followLabel = stringResource(R.string.route_saved_follow)

    // Location services off → offer to open settings (an actionable, persistent
    // state). Slow GPS is NOT surfaced: the first fix just takes time, and a late
    // fix still recentres — no nagging error on startup.
    val locOffMsg = stringResource(R.string.location_off)
    val locOffAction = stringResource(R.string.location_off_action)
    LaunchedEffect(state.locationNotice) {
        when (state.locationNotice) {
            LocationNotice.ServicesOff -> {
                val res = snackbarHostState.showSnackbar(locOffMsg, actionLabel = locOffAction, duration = SnackbarDuration.Long)
                if (res == SnackbarResult.ActionPerformed) {
                    runCatching {
                        context.startActivity(android.content.Intent(android.provider.Settings.ACTION_LOCATION_SOURCE_SETTINGS))
                    }
                }
                viewModel.dismissLocationNotice()
            }
            null -> Unit
        }
    }
    // After a save, offer to immediately follow the just-saved geometry.
    val offerFollowAfterSave: (List<LatLng>, Double, Double, Double) -> Unit = { geo, dist, asc, dur ->
        scope.launch {
            val res = snackbarHostState.showSnackbar(savedToast, actionLabel = followLabel, duration = SnackbarDuration.Short)
            if (res == SnackbarResult.ActionPerformed && geo.size > 1) {
                routeViewModel.followTrack(geo, dist, asc, dur)
                if (viewModel.hasLocationPermission()) {
                    viewModel.enableLocation(); viewModel.setFollowing(true)
                } else {
                    locationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
                }
            }
        }
    }
    // Open the tool fresh in [mode], wiping any half-built geometry.
    val openTrackTool: (TrackMode) -> Unit = { mode ->
        routeViewModel.clear(); ui.linePoints.clear(); ui.drawPoints.clear(); ui.routeOrigin = null
        viewModel.setFollowing(false)
        ui.selectionState.clear()
        ui.selectedWaypoint = null
        ui.trackMode = mode
    }
    // Close the tool; keep the route only when we're handing off to Follow.
    val closeTrackTool: (Boolean) -> Unit = { keepRoute ->
        if (!keepRoute) routeViewModel.clear()
        ui.linePoints.clear(); ui.drawPoints.clear(); ui.routeOrigin = null; ui.trackMode = null
        ui.selectedWaypoint = null
    }
    // Position long-pressed on the map → drives the "new marker" sheet (null = closed).
    // Long-pressed coordinate → the on-map contextual menu (weather + create actions).
    // Marker being edited (null = no editor open).
    // Marker pending delete-confirmation (null = no dialog).
    // Marker whose "add to collection" picker is open (null = closed).

    // Geotagged photos → clustered thumbnail pins on the map; tap a cluster → grid → viewer.
    val photosViewModel: com.sigmundgranaas.turbo.expressive.feature.photos.PhotosViewModel = hiltViewModel()
    val mapPhotos by photosViewModel.onMapPhotos.collectAsStateWithLifecycle()
    val photoClusters = remember(mapPhotos) { com.sigmundgranaas.turbo.expressive.feature.photos.clusterPhotos(mapPhotos) }
    val addPhotoLauncher = rememberLauncherForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia(),
    ) { uri ->
        ui.pendingPhotoAt?.let { at -> if (uri != null) photosViewModel.addFromContent(null, at, uri) }
        ui.pendingPhotoAt = null
    }

    // One selection model + detail host — the map shell no longer depends on the
    // markers feature for the info sheet; it routes through the :core:map seam.
    val actionRegistry = remember { defaultMapEntityActionRegistry() }

    ModalNavigationDrawer(
        drawerState = drawerState,
        // Only intercept swipes while open (swipe-to-close); otherwise the map owns
        // all pan gestures so a left-edge drag pans instead of opening the drawer.
        gesturesEnabled = drawerState.isOpen,
        drawerContent = {
            NavDrawerContent(
                selected = DrawerDestination.Map,
                accountEmail = accountEmail,
                onAccountClick = { scope.launch { drawerState.close() }; onOpenAccount() },
                onSelect = { dest ->
                scope.launch { drawerState.close() }
                when (dest) {
                    DrawerDestination.Settings -> onOpenSettings()
                    DrawerDestination.Paths -> onOpenPaths()
                    DrawerDestination.Collections -> onOpenCollections()
                    DrawerDestination.Record -> startRecording()
                    DrawerDestination.Offline -> onOpenOffline()
                    DrawerDestination.Map -> Unit
                    }
                },
            )
        },
    ) {
        // Marker selection + map-tap behaviour, shared by both renderer hosts so the two
        // paths behave identically (one selection model, one tap state machine).
        val markerSelection: (com.sigmundgranaas.turbo.expressive.domain.Marker) -> MapSelection = { marker ->
            MapSelection(
                id = marker.id,
                title = marker.name,
                subtitle = "${context.getString(marker.kind.labelRes)} · ${formatCoords(marker.position)}",
                icon = marker.kind.icon,
                point = marker.position,
                onNavigate = {
                    // Navigate-to-marker opens the unified Create track tool in Route mode.
                    // If a route is already being built, drop this marker in as a stop.
                    if (ui.trackMode == TrackMode.Route && routeViewModel.waypoints.value.size >= 2) {
                        routeViewModel.addStop(marker.position)
                    } else {
                        val from = state.userLocation ?: ui.controller?.center() ?: MapDefaults.fallbackCamera
                        openTrackTool(TrackMode.Route)
                        routeViewModel.planRoute(from, marker.position)
                    }
                },
                onShare = { shareMarkerGeoJson(context, marker) },
                onEdit = { ui.editingMarker = marker },
                onDelete = { ui.pendingDelete = marker },
                extraActions = listOf(
                    com.sigmundgranaas.turbo.expressive.core.map.MapEntityAction(
                        id = "add_photo",
                        label = context.getString(R.string.marker_add_photo),
                        icon = androidx.compose.material.icons.Icons.Rounded.AddAPhoto,
                        onInvoke = { ui.addPhotoForMarker = marker },
                    ),
                    com.sigmundgranaas.turbo.expressive.core.map.MapEntityAction(
                        id = "add_to_collection",
                        label = context.getString(R.string.marker_add_to_collection),
                        icon = androidx.compose.material.icons.Icons.Rounded.Folder,
                        onInvoke = { ui.addToCollection = marker },
                    ),
                ),
                body = {
                    Column {
                        if (!marker.notes.isNullOrBlank()) {
                            Text(marker.notes!!, style = MaterialTheme.typography.bodyMedium, color = cs.onSurface)
                            Spacer(Modifier.height(14.dp))
                        }
                        ConditionsBody(marker.position)
                        Spacer(Modifier.height(14.dp))
                        com.sigmundgranaas.turbo.expressive.feature.photos.MarkerPhotos(markerId = marker.id)
                    }
                },
            )
        }
        val onMapTapForMode: (LatLng) -> Unit = { p ->
            when (ui.trackMode) {
                TrackMode.Route -> when {
                    // Tap-to-place (US-7): with a stop selected, an empty-map tap RELOCATES it
                    // there and keeps it selected so you can keep refining. Tap its own marker
                    // to deselect. (Marker taps are consumed by the marker, not seen here.)
                    ui.selectedWaypoint != null -> {
                        haptics.toggle(true)
                        routeViewModel.moveWaypointTo(ui.selectedWaypoint!!, p)
                    }
                    // Route exists → each further tap EXTENDS it, in tap order.
                    routeViewModel.waypoints.value.size >= 2 -> { haptics.toggle(true); routeViewModel.appendWaypoint(p) }
                    // First tap = start, second = destination.
                    ui.routeOrigin == null -> { haptics.toggle(true); ui.routeOrigin = p }
                    else -> { haptics.toggle(true); routeViewModel.planRoute(ui.routeOrigin!!, p); ui.routeOrigin = null }
                }
                TrackMode.Line -> { haptics.toggle(true); ui.linePoints.add(p) }
                TrackMode.Draw -> Unit // handled by the drag overlay
                null -> ui.selectionState.clear()
            }
        }
        // wgpu engine failure (no fallback by design): surface it loudly.
        val wgpuError = remember { mutableStateOf<String?>(null) }
        Box(Modifier.fillMaxSize()) {
            // Experimental wgpu renderer (Settings toggle). Renders the basemap +
            // overlays + live track/route/measure/user as a turbomap Scene with
            // pan/zoom; markers / editable waypoints / photo pins / long-press are
            // MapLibre-only for now (see the renderer-swap test plan, Stage E).
            if (state.experimentalWgpuMap) {
                TurbomapMapView(
                    rasters = MapStyles.turbomapRasterSpecs(state.baseLayer, ui.activeOverlays),
                    vectors = MapStyles.turbomapVectorSpecs(),
                    initialCamera = MapDefaults.fallbackCamera,
                    initialZoom = MapDefaults.fallbackZoom,
                    track = when {
                        recState.recording -> recState.points.takeIf { it.size > 1 }
                        ui.trackMode == TrackMode.Line -> ui.linePoints.takeIf { it.size > 1 }?.toList()
                        ui.trackMode == TrackMode.Draw -> ui.drawPoints.takeIf { it.size > 1 }?.toList()
                        else -> ui.displayedTrack
                    },
                    // While solving, don't draw the raw straight line between the waypoints —
                    // only show the path once the solver has streamed real geometry (more points
                    // than the input waypoints). Avoids the "stupid straight line before it's
                    // calculated"; a re-solve keeps showing the previous refined line meanwhile.
                    route = routeState.polyline.takeIf {
                        it.isNotEmpty() && (routeState !is RouteUiState.Solving || it.size > toolWaypoints.size)
                    },
                    measure = when (ui.trackMode) {
                        TrackMode.Line -> ui.linePoints
                        TrackMode.Route -> emptyList() // origin drawn as an origin pin, not a measure dot
                        else -> emptyList()
                    },
                    userLocation = state.userLocation,
                    userHeading = state.userHeading,
                    // 3D mode: 1-finger orbit about the user location, two
                    // fingers pan. Only meaningful on this wgpu engine.
                    threeDMode = state.threeDMode,
                    // In 3D, displace the ground by the real DEM heightmap (the
                    // tileserver's Terrain-RGB). Null in 2D → flat, no DEM fetches.
                    demUrl = if (state.threeDMode) MapStyles.TERRAIN_DEM_URL else null,
                    markers = state.markers,
                    selectedMarkerId = ui.selectionState.selection?.id,
                    photoPins = photoClusters.map {
                        com.sigmundgranaas.turbo.expressive.ui.components.PhotoPin(it.id, it.center.lat, it.center.lng, it.count, it.coverUri)
                    },
                    onPhotoPinClick = { pin -> ui.openCluster = photoClusters.firstOrNull { it.id == pin.id } },
                    waypoints = if (ui.trackMode == TrackMode.Route) toolWaypoints else emptyList(),
                    selectedWaypoint = ui.selectedWaypoint,
                    onWaypointTap = { ui.selectedWaypoint = if (ui.selectedWaypoint == it) null else it },
                    onWaypointLongPress = { haptics.reject(); routeViewModel.removeWaypoint(it); ui.selectedWaypoint = null },
                    onWaypointDragStart = { ui.selectedWaypoint = it; haptics.longPress(); routeViewModel.beginWaypointDrag() },
                    onWaypointMoved = { i, p -> ui.selectedWaypoint = i; routeViewModel.moveWaypointTo(i, p) },
                    onWaypointDragEnd = { routeViewModel.endWaypointDrag() },
                    routeOrigin = if (ui.trackMode == TrackMode.Route) ui.routeOrigin else null,
                    onMarkerClick = { marker -> ui.selectionState.select(markerSelection(marker)) },
                    onMapLongClick = { if (ui.trackMode == null) { haptics.longPress(); ui.longPressAt = it } },
                    onMapTap = { p -> onMapTapForMode(p) },
                    onBearingChange = { ui.bearing = it.toFloat(); ui.cameraIdleTick++ },
                    // A manual pan/zoom/orbit releases camera-follow so it doesn't
                    // snap back to the dot (US-6) — the wgpu engine was missing this.
                    onUserPanned = { viewModel.setFollowing(false) },
                    onMapReady = { ui.controller = it },
                    onEngineError = {
                        wgpuError.value = it
                        android.util.Log.e("TurbomapMap", "wgpu engine error: $it")
                    },
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
            TurboMap(
                base = state.baseLayer,
                overlays = ui.activeOverlays,
                initialCamera = MapDefaults.fallbackCamera,
                initialZoom = MapDefaults.fallbackZoom,
                markers = state.markers,
                // While following, the guide is drawn as two segments split at the arc-cursor:
                // `route` is the bright REMAINING road ahead, `routeCovered` the dim walked part
                // (US-3). The two meet exactly at the cursor so there's no gap or overlap.
                route = if (routeState is RouteUiState.Following) {
                    GeoMetrics.routeSuffix(routeState.polyline, followSession.progress?.fraction ?: 0.0)
                        .takeIf { it.size > 1 } ?: routeState.polyline.takeIf { it.isNotEmpty() }
                } else {
                    // Hide the raw straight line while solving; show it once refined (see wgpu host).
                    routeState.polyline.takeIf {
                        it.isNotEmpty() && (routeState !is RouteUiState.Solving || it.size > toolWaypoints.size)
                    }
                },
                routeCovered = if (routeState is RouteUiState.Following) {
                    GeoMetrics.routePrefix(routeState.polyline, followSession.progress?.fraction ?: 0.0)
                        .takeIf { it.size > 1 }
                } else {
                    null
                },
                // Editable A/B/C… stops while the Route builder is active: tap selects,
                // dragging the selected one moves it, long-press removes it.
                waypoints = if (ui.trackMode == TrackMode.Route) toolWaypoints else emptyList(),
                selectedWaypoint = ui.selectedWaypoint,
                onWaypointTap = { ui.selectedWaypoint = if (ui.selectedWaypoint == it) null else it },
                onWaypointLongPress = {
                    haptics.reject(); routeViewModel.removeWaypoint(it); ui.selectedWaypoint = null
                },
                onWaypointDragStart = { ui.selectedWaypoint = it; haptics.longPress(); routeViewModel.beginWaypointDrag() },
                onWaypointMoved = { i, p -> ui.selectedWaypoint = i; routeViewModel.moveWaypointTo(i, p) },
                onWaypointDragEnd = { routeViewModel.endWaypointDrag() },
                routeOrigin = if (ui.trackMode == TrackMode.Route) ui.routeOrigin else null,
                // While following, draw the checkpoints as on-map markers — crossed ones filled
                // and checked, upcoming ones outlined (US-3).
                checkpoints = if (routeState is RouteUiState.Following) {
                    followSession.phaseMarkers.map { it.position to it.crossed }
                } else {
                    emptyList()
                },
                // The track overlay shows, in priority: the live recording trail, the
                // Line/Draw geometry being built in the Create track tool, else whatever
                // saved track the user opened ("Show on map").
                track = when {
                    recState.recording -> recState.points.takeIf { it.size > 1 }
                    // Follow = Record: show the real travelled line over the dimmed guide.
                    routeState is RouteUiState.Following -> followSession.points.takeIf { it.size > 1 }
                    ui.trackMode == TrackMode.Line -> ui.linePoints.takeIf { it.size > 1 }?.toList()
                    ui.trackMode == TrackMode.Draw -> ui.drawPoints.takeIf { it.size > 1 }?.toList()
                    else -> ui.displayedTrack
                },
                selectedMarkerId = ui.selectionState.selection?.id,
                userLocation = state.userLocation,
                photoPins = photoClusters.map {
                    com.sigmundgranaas.turbo.expressive.ui.components.PhotoPin(it.id, it.center.lat, it.center.lng, it.count, it.coverUri)
                },
                onPhotoPinClick = { pin -> ui.openCluster = photoClusters.firstOrNull { it.id == pin.id } },
                onMarkerClick = { marker -> ui.selectionState.select(markerSelection(marker)) },
                // Dot overlay marks Line vertices, and the pending Route start (the first
                // tap before a destination exists) so it doesn't look like nothing happened.
                measurePoints = when (ui.trackMode) {
                    TrackMode.Line -> ui.linePoints
                    TrackMode.Route -> emptyList() // origin drawn as an origin pin, not a measure dot
                    else -> emptyList()
                },
                onMapLongClick = { if (ui.trackMode == null) { haptics.longPress(); ui.longPressAt = it } },
                onMapTap = onMapTapForMode,
                onMapReady = { ui.controller = it },
                // Fired on every camera-idle; also bump the idle tick so camera-reading
                // chrome (the offline-coverage chip) recomposes after a pan, not only rotation.
                onBearingChange = { ui.bearing = it.toFloat(); ui.cameraIdleTick++ },
                // A manual pan/zoom/rotate releases camera-follow (US-6).
                onUserPanned = { viewModel.setFollowing(false) },
                modifier = Modifier.fillMaxSize(),
            )
            }

            // Procedural weather-cloud overlay — enabled from the Layers sheet
            // (ui.cloudsOn), wgpu engine only. The play/scrub control sits at the
            // BOTTOM of the map (out from under the search bar); it renders
            // nothing while off or on a non-cloud engine.
            if (state.experimentalWgpuMap) {
                RadarOverlayControls(
                    engine = ui.controller,
                    active = ui.cloudsOn,
                    onActiveChange = { ui.cloudsOn = it },
                    source = viewModel.radarSource,
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .navigationBarsPadding()
                        .padding(start = 16.dp, end = 16.dp, bottom = 16.dp),
                )
            }

            // Sun mode — movable sun + atmosphere + cast shadows, with a time-of-
            // day slider at the bottom (defaults to today/now). wgpu engine only;
            // renders nothing when off. Sits above the radar scrubber if both on.
            if (state.experimentalWgpuMap) {
                SunOverlayControls(
                    engine = ui.controller,
                    active = ui.sunModeOn,
                    onActiveChange = { ui.sunModeOn = it },
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .navigationBarsPadding()
                        .padding(
                            start = 16.dp,
                            end = 16.dp,
                            bottom = if (ui.cloudsOn) 76.dp else 16.dp,
                        ),
                )
            }

            // No silent blank: if the wgpu engine failed to start, say so.
            wgpuError.value?.let { msg ->
                Surface(
                    modifier = Modifier.align(Alignment.Center).padding(24.dp),
                    color = cs.errorContainer,
                    shape = MaterialTheme.shapes.large,
                ) {
                    Text(
                        "Map engine failed to start:\n$msg",
                        modifier = Modifier.padding(20.dp),
                        color = cs.onErrorContainer,
                    )
                }
            }

            // Freehand Draw capture: a transparent layer that turns finger drags into
            // track points (and consumes the gesture so the map doesn't pan).
            if (ui.trackMode == TrackMode.Draw) {
                Box(
                    Modifier.fillMaxSize().pointerInput(Unit) {
                        detectDragGestures(
                            onDragStart = { off ->
                                ui.drawPoints.clear()
                                ui.controller?.fromScreen(off.x, off.y)?.let { ui.drawPoints.add(it) }
                            },
                            onDrag = { change, _ ->
                                ui.controller?.fromScreen(change.position.x, change.position.y)?.let { ui.drawPoints.add(it) }
                            },
                        )
                    },
                )
            }

            // Chrome is hidden while the Create track tool owns the screen.
            if (ui.trackMode == null) {
                SearchPill(
                    placeholder = "Search places, coordinates…",
                    onMenuClick = { scope.launch { drawerState.open() } },
                    onClick = onOpenSearch,
                    modifier = Modifier
                        .align(Alignment.TopCenter)
                        .windowInsetsPadding(WindowInsets.statusBars)
                        .padding(16.dp),
                )

                // Slow-GPS feedback: a small "locating…" pill under the search bar while
                // we wait for the first fix, so a slow/cold start doesn't look frozen.
                if (state.locating) {
                    LocatingChip(
                        modifier = Modifier
                            .align(Alignment.TopCenter)
                            .windowInsetsPadding(WindowInsets.statusBars)
                            .padding(top = 84.dp),
                    )
                } else if (isOffline) {
                    // Why-is-the-map-blank affordance: offline, and (stronger) outside
                    // every downloaded region. Re-checked on each camera-idle via the tick.
                    @Suppress("UNUSED_EXPRESSION") ui.cameraIdleTick
                    val centre = ui.controller?.center()
                    val outside = centre != null && !offlineIndicator.covered(centre)
                    OfflineChip(
                        outsideCoverage = outside,
                        modifier = Modifier
                            .align(Alignment.TopCenter)
                            .windowInsetsPadding(WindowInsets.statusBars)
                            .padding(top = 84.dp),
                    )
                }

                // The live sheet owns the bottom of the screen; reserve its height so
                // the centred rail (esp. the zoom cookies) lifts clear of it instead of
                // hiding behind it. Capped so a fully-expanded sheet can't push the rail
                // off the top.
                val liveSheetShown = recState.recording ||
                    (routeState is RouteUiState.Following && ui.trackMode == null)
                val railBottomReserve = if (liveSheetShown) {
                    val maxH = configuration.screenHeightDp.dp
                    val raw = when (liveDetent) {
                        com.sigmundgranaas.turbo.expressive.feature.map.live.LiveDetent.Mini -> minOf(208.dp, maxH * 0.64f)
                        com.sigmundgranaas.turbo.expressive.feature.map.live.LiveDetent.Peek -> minOf(340.dp, maxH * 0.64f)
                        com.sigmundgranaas.turbo.expressive.feature.map.live.LiveDetent.Half -> maxH * 0.56f
                        com.sigmundgranaas.turbo.expressive.feature.map.live.LiveDetent.Full -> maxH * 0.92f
                    }
                    raw.coerceAtMost(maxH * 0.5f)
                } else {
                    0.dp
                }
                MapControlRail(
                    following = state.following,
                    creatingTrack = false,
                    bearing = ui.bearing,
                    onCompass = { ui.controller?.resetNorth() },
                    onAdd = { (ui.controller?.center() ?: state.userLocation)?.let { ui.newMarkerAt = it } },
                    onCreateTrack = {
                        // Don't silently drop an active follow when launching the build tool.
                        if (routeState is RouteUiState.Following) ui.confirmReplaceFollow = true
                        else openTrackTool(TrackMode.Route)
                    },
                    onLayers = { ui.showLayers = true },
                    onLocate = {
                        if (viewModel.hasLocationPermission()) {
                            viewModel.enableLocation()
                            val next = !state.following
                            viewModel.setFollowing(next)
                            if (next) state.userLocation?.let { ui.controller?.flyTo(it, 15.0) }
                        } else {
                            locationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
                        }
                    },
                    onZoomIn = { ui.controller?.zoomIn() },
                    onZoomOut = { ui.controller?.zoomOut() },
                    // 2D/3D toggle — only on the wgpu engine (the only one with
                    // orbit + tilt). Hidden on MapLibre.
                    threeD = state.threeDMode,
                    onToggle3D = if (state.experimentalWgpuMap) {
                        { viewModel.setThreeDMode(!state.threeDMode) }
                    } else {
                        null
                    },
                    // Sun mode: movable sun + atmosphere + cast shadows (wgpu only).
                    // Turning it on also flips to 3D — the relief, sky and shadows
                    // only read under tilt.
                    sunMode = ui.sunModeOn,
                    onToggleSun = if (state.experimentalWgpuMap) {
                        {
                            val next = !ui.sunModeOn
                            ui.sunModeOn = next
                            if (next && !state.threeDMode) viewModel.setThreeDMode(true)
                        }
                    } else {
                        null
                    },
                    modifier = Modifier
                        .align(Alignment.CenterEnd)
                        .windowInsetsPadding(WindowInsets.statusBars)
                        .padding(end = 14.dp, bottom = railBottomReserve),
                )
            }

            // (The "Following" pill was removed — the locate rail button already
            //  recolours when follow is active, which is the only indicator needed.)

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
                // The music-app-style live sheet (peek → half → full) owns the bottom
                // slot while recording, reading the same LiveStats the lock-screen widget shows.
                com.sigmundgranaas.turbo.expressive.feature.map.live.LiveSheet(
                    stats = com.sigmundgranaas.turbo.expressive.core.data.LiveStats.of(recSession),
                    metric = metric,
                    title = com.sigmundgranaas.turbo.expressive.feature.map.live.formatLiveClock(recSession.elapsedSec),
                    detent = ui.recDetent,
                    onDetentChange = { ui.recDetent = it },
                    onTogglePause = {
                        haptics.toggle(recState.paused)
                        when {
                            !recSession.paused -> recordingViewModel.pause()
                            // Forgot to unpause and kept walking? Ask before resuming (US-4).
                            recSession.hasBufferedMovement -> ui.showResumeBufferPrompt = true
                            else -> recordingViewModel.resume(include = false)
                        }
                    },
                    onStop = { haptics.confirm(); recordingViewModel.stop(); ui.showRecSave = true },
                    elevations = recSession.elevations.filterNotNull(),
                    modifier = Modifier.align(Alignment.BottomCenter),
                )
            } else if (routeState is RouteUiState.Following && ui.trackMode == null) {
                // Following a route → the same live sheet, in follow mode.
                val followName = followSession.name
                com.sigmundgranaas.turbo.expressive.feature.map.live.LiveSheet(
                    stats = com.sigmundgranaas.turbo.expressive.core.data.LiveStats.of(followSession),
                    metric = metric,
                    title = followName ?: stringResource(R.string.route_following),
                    detent = ui.followDetent,
                    onDetentChange = { ui.followDetent = it },
                    onTogglePause = {
                        haptics.toggle(followSession.paused)
                        when {
                            !followSession.paused -> routeViewModel.pauseFollow()
                            // Forgot to unpause and kept walking? Ask before resuming (US-4).
                            followSession.hasBufferedMovement -> ui.showFollowResumeBufferPrompt = true
                            else -> routeViewModel.resumeFollow(include = false)
                        }
                    },
                    onStop = { ui.showFollowStopSave = true },
                    nextWaypoint = followName?.let {
                        stringResource(
                            R.string.live_next_waypoint,
                            com.sigmundgranaas.turbo.expressive.core.geo.Units.distance(
                                followSession.progress?.distanceRemainingM ?: (followSession.plan?.distanceM ?: 0.0), metric,
                            ),
                        ) to it
                    },
                    modifier = Modifier.align(Alignment.BottomCenter),
                )
            } else if (ui.trackMode == null) {
                val routeWaypoints by routeViewModel.waypoints.collectAsStateWithLifecycle()
                RouteCard(
                    state = routeState,
                    preset = routePreset,
                    waypointCount = routeWaypoints.size,
                    onRemoveStop = { index -> routeViewModel.removeWaypoint(index) },
                    onSelectPreset = { routeViewModel.selectPreset(it) },
                    onFollow = {
                        if (viewModel.hasLocationPermission()) {
                            viewModel.enableLocation()
                            viewModel.setFollowing(true)
                            routeViewModel.follow(nearbyCheckpoints(routeState, state.markers))
                        } else {
                            locationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
                        }
                    },
                    onSave = { ui.showRouteSave = true },
                    onDownloadOffline = { routeViewModel.downloadAlongRoute(state.baseLayer) },
                    onClear = {
                        // Stopping a follow offers to keep it (→ Saved Tracks history);
                        // any other state just clears.
                        if (routeState is RouteUiState.Following) {
                            ui.showFollowStopSave = true
                        } else {
                            routeViewModel.clear(); viewModel.setFollowing(false)
                        }
                    },
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
            ui.trackMode?.let { mode ->
                val donePlan = (routeState as? RouteUiState.Done)?.plan
                val geometry = when (mode) {
                    TrackMode.Route -> routeState.polyline
                    TrackMode.Line -> ui.linePoints.toList()
                    TrackMode.Draw -> ui.drawPoints.toList()
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
                    TrackMode.Line -> if (ui.linePoints.isNotEmpty()) stringResource(R.string.track_meta_line, ui.linePoints.size) else stringResource(R.string.track_meta_empty)
                    TrackMode.Draw -> if (ui.drawPoints.size >= 2) stringResource(R.string.track_meta_draw) else stringResource(R.string.track_meta_empty)
                }
                val canSave = if (mode == TrackMode.Route) routeState is RouteUiState.Done else geometry.size >= 2
                val canUndo = when (mode) {
                    TrackMode.Route -> routeViewModel.canUndo
                    TrackMode.Line -> ui.linePoints.isNotEmpty()
                    TrackMode.Draw -> ui.drawPoints.isNotEmpty()
                }
                // Anything the user has placed that a close would throw away.
                val hasUnsaved = when (mode) {
                    TrackMode.Route -> routeViewModel.waypoints.value.isNotEmpty() || ui.routeOrigin != null
                    TrackMode.Line -> ui.linePoints.isNotEmpty()
                    TrackMode.Draw -> ui.drawPoints.isNotEmpty()
                }

                CreateTrackCloseButton(
                    onClose = { if (hasUnsaved) ui.showTrackDiscard = true else closeTrackTool(false) },
                    modifier = Modifier.align(Alignment.TopStart)
                        .windowInsetsPadding(WindowInsets.statusBars).padding(16.dp),
                )
                // Keep zoom + recenter reachable — the full rail is hidden in the tool.
                CreateTrackMapControls(
                    following = state.following,
                    onLocate = {
                        if (viewModel.hasLocationPermission()) {
                            viewModel.enableLocation()
                            state.userLocation?.let { ui.controller?.flyTo(it, 15.0) }
                        } else {
                            locationPermission.launch(Manifest.permission.ACCESS_FINE_LOCATION)
                        }
                    },
                    onZoomIn = { ui.controller?.zoomIn() },
                    onZoomOut = { ui.controller?.zoomOut() },
                    modifier = Modifier.align(Alignment.CenterEnd)
                        .windowInsetsPadding(WindowInsets.statusBars).padding(end = 14.dp),
                )
                Column(
                    Modifier.align(Alignment.BottomCenter)
                        // Cap to a phone-comfortable width so the panel doesn't stretch
                        // edge-to-edge on landscape/tablet (centered there).
                        .responsiveContentWidth(560.dp)
                        .windowInsetsPadding(WindowInsets.navigationBars).padding(16.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    // While a planned route is re-solving after an edit, show "Updating
                    // route…" (the old line stays on the map). No standing coachmark —
                    // the panel itself is the affordance.
                    if (mode == TrackMode.Route && routeState is RouteUiState.Solving && toolWaypoints.size >= 2) {
                        CreateTrackUpdatingChip()
                        Spacer(Modifier.height(12.dp))
                    }
                    CreateTrackPanel(
                        mode = mode,
                        onMode = { next -> haptics.toggle(true); ui.trackMode = next },
                        distanceText = distText,
                        unit = unitText,
                        metaText = meta,
                        surfaces = donePlan?.surfaces ?: emptyMap(),
                        presetLabel = routePreset.label,
                        onRouteStyle = { ui.showRouteStyle = true },
                        stopCount = if (mode == TrackMode.Route) toolWaypoints.size else 0,
                        onManageStops = { ui.showStops = true },
                        canUndo = canUndo,
                        canSave = canSave,
                        onUndo = {
                            when (mode) {
                                TrackMode.Route -> routeViewModel.undo()
                                TrackMode.Line -> if (ui.linePoints.isNotEmpty()) ui.linePoints.removeAt(ui.linePoints.lastIndex)
                                TrackMode.Draw -> ui.drawPoints.clear()
                            }
                        },
                        onClear = {
                            when (mode) {
                                TrackMode.Route -> { routeViewModel.clear(); ui.routeOrigin = null }
                                TrackMode.Line -> ui.linePoints.clear()
                                TrackMode.Draw -> ui.drawPoints.clear()
                            }
                        },
                        onSave = { haptics.toggle(true); ui.showTrackSave = true },
                        onFollow = {
                            haptics.confirm()
                            if (mode == TrackMode.Route) routeViewModel.follow(nearbyCheckpoints(routeState, state.markers))
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

                if (ui.showRouteStyle) {
                    RouteStyleSheet(
                        selected = routePreset,
                        onSelect = { routeViewModel.selectPreset(it); ui.showRouteStyle = false },
                        onDismiss = { ui.showRouteStyle = false },
                    )
                }
                if (ui.showStops) {
                    WaypointsSheet(
                        waypoints = toolWaypoints,
                        statText = meta,
                        onMove = { from, to -> routeViewModel.moveWaypoint(from, to) },
                        onRemove = { routeViewModel.removeWaypoint(it) },
                        onAddStop = { ui.showStops = false },
                        onDismiss = { ui.showStops = false },
                    )
                }
                if (ui.showTrackSave) {
                    NameInputDialog(
                        title = stringResource(R.string.track_save_title),
                        confirmLabel = stringResource(com.sigmundgranaas.turbo.expressive.core.designsystem.R.string.ds_save),
                        initial = if (full.isNotBlank()) "Track $full" else "Track",
                        onConfirm = { name ->
                            // Capture before closing wipes the working geometry, so the
                            // "Saved · Follow" snackbar can follow the just-saved line.
                            val savedGeo = geometry
                            val savedDist = distM
                            val savedAsc = donePlan?.ascentM ?: 0.0
                            val savedDur = donePlan?.durationS ?: 0.0
                            if (mode == TrackMode.Route) routeViewModel.saveAsTrack(name)
                            else routeViewModel.saveLine(name, savedGeo)
                            ui.showTrackSave = false
                            closeTrackTool(false)
                            offerFollowAfterSave(savedGeo, savedDist, savedAsc, savedDur)
                        },
                        onDismiss = { ui.showTrackSave = false },
                    )
                }
                if (ui.showTrackDiscard) {
                    com.sigmundgranaas.turbo.expressive.ui.components.TurboConfirmDialog(
                        title = stringResource(R.string.track_discard_title),
                        body = stringResource(R.string.track_discard_body),
                        confirmLabel = stringResource(R.string.track_discard),
                        icon = androidx.compose.material.icons.Icons.Rounded.DeleteSweep,
                        destructive = true,
                        onConfirm = { ui.showTrackDiscard = false; closeTrackTool(false) },
                        onDismiss = { ui.showTrackDiscard = false },
                    )
                }
            }

            // On-map contextual menu blooming at a long-press (weather + create actions).
            // Kept mounted through its exit animation via a transition state + last point,
            // so the bloom can scale/fade back out after longPressAt clears.
            val lpVisible = remember { MutableTransitionState(false) }
            val lpShown = remember { mutableStateOf<LatLng?>(null) }
            LaunchedEffect(ui.longPressAt) {
                val target = ui.longPressAt
                if (target != null) { lpShown.value = target; lpVisible.targetState = true }
                else lpVisible.targetState = false
            }
            LaunchedEffect(lpVisible.isIdle, lpVisible.currentState) {
                if (lpVisible.isIdle && !lpVisible.currentState) lpShown.value = null
            }
            lpShown.value?.let { p ->
                ui.controller?.let { ctrl ->
                    val (sx, sy) = ctrl.toScreen(p)
                    // Reverse-geocode the pressed point so the header reads "On Storfjellet"
                    // rather than raw coordinates (same label the marker editor shows).
                    val lpDescription by viewModel.pointDescription.collectAsStateWithLifecycle()
                    LaunchedEffect(p) { viewModel.describePoint(p) }
                    MapLongPressMenu(
                        visibleState = lpVisible,
                        point = p,
                        anchor = androidx.compose.ui.geometry.Offset(sx, sy),
                        placeLabel = lpDescription?.label,
                        onNewMarker = { ui.longPressAt = null; ui.newMarkerAt = p },
                        onRouteHere = {
                            ui.longPressAt = null
                            val from = state.userLocation ?: ctrl.center()
                            openTrackTool(TrackMode.Route)
                            routeViewModel.planRoute(from, p)
                        },
                        onStartRouteHere = {
                            // Begin a route whose FIRST waypoint is this point: enter Route mode
                            // and drop the origin (shown as an origin pin). The next tap sets the
                            // destination → planRoute, exactly like tapping the origin on the map.
                            ui.longPressAt = null
                            haptics.toggle(true)
                            openTrackTool(TrackMode.Route)
                            ui.routeOrigin = p
                            ui.selectedWaypoint = null
                        },
                        onCreateTrack = { ui.longPressAt = null; openTrackTool(TrackMode.Route) },
                        onOpenForecast = { ui.longPressAt = null; ui.forecastAt = p },
                        onAddPhoto = {
                            ui.longPressAt = null
                            ui.pendingPhotoAt = p
                            addPhotoLauncher.launch(
                                androidx.activity.result.PickVisualMediaRequest(
                                    androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia.ImageOnly,
                                ),
                            )
                        },
                        onDismiss = { ui.longPressAt = null },
                    )
                }
            }

            // Photo cluster → grid sheet → immersive viewer.
            ui.openCluster?.let { cluster ->
                com.sigmundgranaas.turbo.expressive.feature.photos.PhotoClusterSheet(
                    cluster = cluster,
                    onOpen = { index -> ui.viewerStart = index },
                    onDismiss = { ui.openCluster = null },
                )
                if (ui.viewerStart >= 0) {
                    com.sigmundgranaas.turbo.expressive.feature.photos.PhotoViewer(
                        photos = cluster.ordered,
                        startIndex = ui.viewerStart,
                        onClose = { ui.viewerStart = -1 },
                        onDelete = { photo -> photosViewModel.delete(photo); ui.viewerStart = -1; ui.openCluster = null },
                    )
                }
            }

            // The Save → Follow bridge snackbar floats above the map (bottom).
            SnackbarHost(
                hostState = snackbarHostState,
                modifier = Modifier.align(Alignment.BottomCenter)
                    .windowInsetsPadding(WindowInsets.navigationBars).padding(16.dp),
            )
        }
    }

    // The map's overlay layer of position-independent popups (detail host + dialogs +
    // bottom sheets) lives in MapScreenModals — the screen body keeps the camera +
    // content orchestration; that file owns the modal wiring.
    MapScreenModals(
        ui = ui,
        viewModel = viewModel,
        routeViewModel = routeViewModel,
        recordingViewModel = recordingViewModel,
        offlineViewModel = offlineViewModel,
        actionRegistry = actionRegistry,
        metric = metric,
        routeState = routeState,
        recDistanceM = recState.distanceM,
        recPointCount = recState.points.size,
        baseLayer = state.baseLayer,
        cloudsAvailable = state.experimentalWgpuMap,
        onOpenOffline = onOpenOffline,
        openTrackTool = openTrackTool,
    )
}


/** How close a saved marker must sit to the planned route to count as a checkpoint (D3). */
private const val CHECKPOINT_NEAR_M = 40.0

/**
 * Saved markers within [CHECKPOINT_NEAR_M] of the solved route, as (position, name) checkpoints
 * (D3). [RouteViewModel.follow] merges these with the route stops and orders both by arc-length.
 */
private fun nearbyCheckpoints(state: RouteUiState, markers: List<Marker>): List<Pair<LatLng, String>> {
    val geometry = (state as? RouteUiState.Done)?.plan?.geometry ?: return emptyList()
    return markers
        .filter { GeoMetrics.distanceToPath(geometry, it.position) <= CHECKPOINT_NEAR_M }
        .map { it.position to it.name }
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

/** Small pill shown under the search bar while waiting for the first GPS fix. */
@Composable
private fun LocatingChip(modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(50),
        color = cs.surfaceContainerHigh,
        shadowElevation = 3.dp,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
        ) {
            CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp, color = cs.primary)
            Spacer(Modifier.width(10.dp))
            Text(
                stringResource(R.string.location_finding),
                style = MaterialTheme.typography.labelLarge,
                color = cs.onSurface,
            )
        }
    }
}

/** "You're offline" pill under the search bar; says so louder when the camera is
 *  also outside every downloaded region (i.e. the basemap will be blank). */
@Composable
private fun OfflineChip(outsideCoverage: Boolean, modifier: Modifier = Modifier) {
    val cs = MaterialTheme.colorScheme
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(50),
        color = if (outsideCoverage) cs.errorContainer else cs.surfaceContainerHigh,
        shadowElevation = 3.dp,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
        ) {
            Icon(
                Icons.Rounded.CloudOff,
                null,
                tint = if (outsideCoverage) cs.onErrorContainer else cs.primary,
                modifier = Modifier.size(16.dp),
            )
            Spacer(Modifier.width(10.dp))
            Text(
                stringResource(if (outsideCoverage) R.string.offline_chip_uncovered else R.string.offline_chip),
                style = MaterialTheme.typography.labelLarge,
                color = if (outsideCoverage) cs.onErrorContainer else cs.onSurface,
            )
        }
    }
}
