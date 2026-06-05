package com.sigmundgranaas.turbo.expressive.feature.map

import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.material.icons.rounded.Bookmark
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import com.sigmundgranaas.turbo.expressive.feature.markers.NewMarkerSheet
import com.sigmundgranaas.turbo.expressive.feature.nav.DrawerDestination
import com.sigmundgranaas.turbo.expressive.feature.nav.NavDrawerContent
import com.sigmundgranaas.turbo.expressive.ui.components.DeleteMarkerDialog
import com.sigmundgranaas.turbo.expressive.ui.components.MapControlRail
import com.sigmundgranaas.turbo.expressive.ui.components.SearchPill
import com.sigmundgranaas.turbo.expressive.ui.components.SectionLabel
import com.sigmundgranaas.turbo.expressive.ui.map.MapController
import com.sigmundgranaas.turbo.expressive.ui.map.TurboMap
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import com.sigmundgranaas.turbo.expressive.ui.theme.icon
import kotlin.math.roundToInt
import kotlinx.coroutines.launch

@Composable
fun MapScreen(
    onOpenSearch: () -> Unit,
    onOpenSettings: () -> Unit,
    onOpenRecording: () -> Unit,
    onOpenPaths: () -> Unit,
    focusRequest: LatLng? = null,
    onFocusConsumed: () -> Unit = {},
    viewModel: MapViewModel = hiltViewModel(),
    routeViewModel: RouteViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val state by viewModel.state.collectAsStateWithLifecycle()
    val routeState by routeViewModel.state.collectAsStateWithLifecycle()
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
                                val from = state.userLocation ?: controller?.center() ?: SampleData.initialCamera
                                routeViewModel.planRoute(from, marker.position)
                            },
                            onDelete = { pendingDelete = marker },
                            body = { ConditionsBody(marker.position) },
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

            RouteCard(
                state = routeState,
                onSave = { routeViewModel.saveAsTrack("Route") },
                onClear = { routeViewModel.clear() },
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
            onDismiss = { showLayers = false },
        )
    }
    // New marker: opened by a long-press on the map, anchored at that coordinate.
    newMarkerAt?.let { pos ->
        NewMarkerSheet(
            position = pos,
            onDismiss = { newMarkerAt = null },
            onSave = { name, kind ->
                viewModel.addMarker(name, kind, pos)
                newMarkerAt = null
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

/** Bottom card reflecting the route solve: solving spinner, result stats, or error. */
@Composable
private fun RouteCard(
    state: RouteUiState,
    onSave: () -> Unit,
    onClear: () -> Unit,
    modifier: Modifier = Modifier,
) {
    if (state is RouteUiState.Idle) return
    val cs = MaterialTheme.colorScheme
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(TurboRadius.xl),
        color = cs.surfaceContainerHigh,
        shadowElevation = 4.dp,
    ) {
        Column(Modifier.padding(horizontal = 18.dp, vertical = 14.dp)) {
            when (state) {
                is RouteUiState.Solving -> Row(verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(modifier = Modifier.size(22.dp), strokeWidth = 2.5.dp)
                    Spacer(Modifier.width(14.dp))
                    Text("Finding the best route…", style = MaterialTheme.typography.titleMedium, color = cs.onSurface, modifier = Modifier.weight(1f))
                    TextButton(onClick = onClear) { Text("Cancel") }
                }
                is RouteUiState.Error -> Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(state.message, style = MaterialTheme.typography.bodyMedium, color = cs.onSurface, modifier = Modifier.weight(1f))
                    TextButton(onClick = onClear) { Text("Dismiss") }
                }
                is RouteUiState.Done -> {
                    val p = state.plan
                    Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
                        RouteStat("%.1f km".format(p.distanceM / 1000.0), "Distance")
                        RouteStat(formatDuration(p.durationS), "Time")
                        RouteStat("${p.ascentM.roundToInt()} m", "Ascent")
                        RouteStat("${p.onTrailPct.roundToInt()}%", "On trail")
                    }
                    Spacer(Modifier.height(12.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Button(onClick = onSave, modifier = Modifier.weight(1f)) {
                            Icon(Icons.Rounded.Bookmark, null, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.width(8.dp))
                            Text("Save as track")
                        }
                        TextButton(onClick = onClear) { Text("Clear") }
                    }
                }
                RouteUiState.Idle -> Unit
            }
        }
    }
}

@Composable
private fun RouteStat(value: String, label: String) {
    val cs = MaterialTheme.colorScheme
    Column {
        Text(value, style = MaterialTheme.typography.titleMedium, color = cs.onSurface)
        Text(label.uppercase(), style = MaterialTheme.typography.labelSmall, color = cs.onSurfaceVariant)
    }
}

private fun formatDuration(seconds: Double): String {
    val total = seconds.roundToInt()
    val h = total / 3600
    val m = (total % 3600) / 60
    return if (h > 0) "${h}h ${m}m" else "$m min"
}
