package com.sigmundgranaas.turbo.expressive.feature.map

import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
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
import androidx.compose.material.icons.rounded.MyLocation
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
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
import com.sigmundgranaas.turbo.expressive.ui.components.DeleteMarkerDialog
import com.sigmundgranaas.turbo.expressive.ui.components.MapControlRail
import com.sigmundgranaas.turbo.expressive.ui.components.SearchPill
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.map.MapController
import com.sigmundgranaas.turbo.expressive.ui.map.TurboMap
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import com.sigmundgranaas.turbo.expressive.ui.theme.icon
import kotlinx.coroutines.launch

/** How far off the planned line (metres) before we re-solve while following. */
private const val OFF_ROUTE_THRESHOLD_M = 50.0

@Composable
fun MapScreen(
    onOpenSearch: () -> Unit,
    onOpenSettings: () -> Unit,
    onOpenRecording: () -> Unit,
    onOpenPaths: () -> Unit,
    onOpenOffline: () -> Unit,
    focusRequest: LatLng? = null,
    onFocusConsumed: () -> Unit = {},
    viewModel: MapViewModel = hiltViewModel(),
    routeViewModel: RouteViewModel = hiltViewModel(),
    offlineViewModel: OfflineViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val state by viewModel.state.collectAsStateWithLifecycle()
    val routeState by routeViewModel.state.collectAsStateWithLifecycle()
    val routePreset by routeViewModel.preset.collectAsStateWithLifecycle()
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()

    var controller by remember { mutableStateOf<MapController?>(null) }

    val locationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        if (granted) {
            viewModel.enableLocation()
            viewModel.setFollowing(true)
        }
    }

    // Show the user's location on first load if the permission is already granted.
    LaunchedEffect(Unit) { if (viewModel.hasLocationPermission()) viewModel.enableLocation() }

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
    // Position long-pressed on the map → drives the "new marker" sheet (null = closed).
    var newMarkerAt by remember { mutableStateOf<LatLng?>(null) }
    // Marker being edited (null = no editor open).
    var editingMarker by remember { mutableStateOf<Marker?>(null) }
    // Marker pending delete-confirmation (null = no dialog).
    var pendingDelete by remember { mutableStateOf<Marker?>(null) }

    // One selection model + detail host — the map shell no longer depends on the
    // markers feature for the info sheet; it routes through the :core:map seam.
    val selectionState = remember { MapSelectionState() }
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
                    DrawerDestination.Record -> onOpenRecording()
                    DrawerDestination.Offline -> onOpenOffline()
                    DrawerDestination.Map -> Unit
                }
            }
        },
    ) {
        Box(Modifier.fillMaxSize()) {
            TurboMap(
                base = state.baseLayer,
                initialCamera = SampleData.initialCamera,
                initialZoom = SampleData.initialZoom,
                markers = state.markers,
                route = routeState.polyline.takeIf { it.isNotEmpty() },
                selectedMarkerId = selectionState.selection?.id,
                userLocation = state.userLocation,
                onMarkerClick = { marker ->
                    selectionState.select(
                        MapSelection(
                            id = marker.id,
                            title = marker.name,
                            subtitle = "${marker.kind.label} · ${formatCoords(marker.position)}",
                            icon = marker.kind.icon,
                            point = marker.position,
                            onNavigate = {
                                // If a route is already up, drop this marker in as a stop;
                                // otherwise start a fresh route from here to the marker.
                                if (routeState is RouteUiState.Done || routeState is RouteUiState.Following) {
                                    routeViewModel.addStop(marker.position)
                                } else {
                                    val from = state.userLocation ?: controller?.center() ?: SampleData.initialCamera
                                    routeViewModel.planRoute(from, marker.position)
                                }
                            },
                            onEdit = { editingMarker = marker },
                            onDelete = { pendingDelete = marker },
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
                                    ConditionsBody(marker.position)
                                }
                            },
                        ),
                    )
                },
                onMapLongClick = { newMarkerAt = it },
                onMapReady = { controller = it },
                modifier = Modifier.fillMaxSize(),
            )

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
                onSave = { routeViewModel.saveAsTrack("Route") },
                onClear = { routeViewModel.clear(); viewModel.setFollowing(false) },
                modifier = Modifier.align(Alignment.BottomCenter)
                    .windowInsetsPadding(WindowInsets.navigationBars)
                    .padding(16.dp),
            )
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
            onDismiss = { showLayers = false },
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
}

