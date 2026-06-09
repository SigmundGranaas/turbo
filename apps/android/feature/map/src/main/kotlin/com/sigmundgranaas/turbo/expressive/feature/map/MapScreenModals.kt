package com.sigmundgranaas.turbo.expressive.feature.map

import android.widget.Toast
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Route
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import com.sigmundgranaas.turbo.expressive.core.map.MapEntityActionRegistry
import com.sigmundgranaas.turbo.expressive.core.map.MapEntityDetailHost
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.CollectionItemType
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.feature.collectionpicker.CollectionPickerSheet
import com.sigmundgranaas.turbo.expressive.feature.conditions.WeatherForecastSheet
import com.sigmundgranaas.turbo.expressive.feature.layers.MapLayersSheet
import com.sigmundgranaas.turbo.expressive.feature.markers.MarkerEditorSheet
import com.sigmundgranaas.turbo.expressive.feature.recording.RecordingViewModel
import com.sigmundgranaas.turbo.expressive.feature.recording.TrackSaveDialog
import com.sigmundgranaas.turbo.expressive.feature.offline.DownloadAreaDialog
import com.sigmundgranaas.turbo.expressive.feature.offline.OfflineViewModel
import com.sigmundgranaas.turbo.expressive.ui.components.DeleteMarkerDialog
import com.sigmundgranaas.turbo.expressive.ui.components.NameInputDialog
import com.sigmundgranaas.turbo.expressive.ui.components.TurboConfirmDialog

/**
 * The map's overlay layer of position-independent popups: the entity detail host plus
 * every dialog / bottom sheet that renders when its trigger state is set. Reads and
 * mutates the shared [MapScreenState] directly, so [MapScreen] keeps the camera +
 * content orchestration and this file owns the modal wiring.
 */
@Suppress("LongMethod") // a cohesive modal cluster — one block per popup
@Composable
internal fun MapScreenModals(
    ui: MapScreenState,
    viewModel: MapViewModel,
    routeViewModel: RouteViewModel,
    recordingViewModel: RecordingViewModel,
    offlineViewModel: OfflineViewModel,
    actionRegistry: MapEntityActionRegistry,
    metric: Boolean,
    routeState: RouteUiState,
    recDistanceM: Double,
    recPointCount: Int,
    baseLayer: BaseLayer,
    onOpenOffline: () -> Unit,
    openTrackTool: (TrackMode) -> Unit,
) {
    val context = LocalContext.current

    // ---- Selection detail host (markers, and any future entity) ----
    MapEntityDetailHost(state = ui.selectionState, registry = actionRegistry)

    // Full weather/ocean forecast for a long-pressed point (its weather readout was tapped).
    ui.forecastAt?.let { point ->
        WeatherForecastSheet(point = point, onDismiss = { ui.forecastAt = null })
    }

    // Stopping a follow: keep it (→ Saved Tracks) or discard. Mirrors recording's
    // save-on-stop, so a route you followed can land in your history.
    if (ui.showFollowStopSave) {
        val plan = (routeState as? RouteUiState.Following)?.plan
        TrackSaveDialog(
            defaultName = "Route ${Units.distance(plan?.distanceM ?: 0.0, metric)}",
            canSave = (plan?.geometry?.size ?: 0) > 1,
            onSave = { name, kind ->
                routeViewModel.saveAsTrack(name, kind)
                Toast.makeText(context, R.string.route_saved, Toast.LENGTH_SHORT).show()
                ui.showFollowStopSave = false
                routeViewModel.clear(); viewModel.setFollowing(false)
            },
            onDiscard = { ui.showFollowStopSave = false; routeViewModel.clear(); viewModel.setFollowing(false) },
            onDismiss = { ui.showFollowStopSave = false }, // keep following
        )
    }
    // Launching the build tool while following would drop the active route — confirm.
    if (ui.confirmReplaceFollow) {
        TurboConfirmDialog(
            title = stringResource(R.string.route_replace_follow_title),
            body = stringResource(R.string.route_replace_follow_body),
            confirmLabel = stringResource(R.string.route_replace_follow_confirm),
            icon = Icons.Rounded.Route,
            onConfirm = { ui.confirmReplaceFollow = false; openTrackTool(TrackMode.Route) },
            onDismiss = { ui.confirmReplaceFollow = false },
        )
    }

    // ---- Tool sheets ----
    if (ui.showLayers) {
        MapLayersSheet(
            selected = baseLayer,
            onSelectBase = viewModel::setBaseLayer,
            onDownloadArea = {
                ui.controller?.let { ctrl ->
                    val bounds = ctrl.visibleBounds()
                    val centre = LatLng((bounds.north + bounds.south) / 2, (bounds.east + bounds.west) / 2)
                    // Capture the viewport and confirm the size before committing (see below).
                    ui.pendingDownloadArea = PendingDownloadArea(bounds, centre, ctrl.zoom())
                }
                ui.showLayers = false
            },
            activeOverlays = ui.activeOverlays,
            onToggleOverlay = { id, on -> ui.activeOverlays = if (on) ui.activeOverlays + id else ui.activeOverlays - id },
            onDismiss = { ui.showLayers = false },
        )
    }
    // Pre-flight size confirm for "Download this area": shows the estimate and blocks
    // areas too large to download (zoom in instead). Confirm commits + opens Offline.
    ui.pendingDownloadArea?.let { area ->
        val estimate = remember(area, baseLayer) {
            offlineViewModel.estimate(baseLayer, area.bounds, area.zoom)
        }
        DownloadAreaDialog(
            estimate = estimate,
            onConfirm = {
                offlineViewModel.download(area.centre, baseLayer, area.bounds, area.zoom)
                ui.pendingDownloadArea = null
                onOpenOffline()
            },
            onDismiss = { ui.pendingDownloadArea = null },
        )
    }
    // Add-to-collection picker for the selected marker.
    ui.addToCollection?.let { marker ->
        CollectionPickerSheet(
            itemId = marker.id,
            type = CollectionItemType.Marker,
            onDismiss = { ui.addToCollection = null },
        )
    }
    // New marker: opened by a long-press on the map, anchored at that coordinate.
    // The coordinate is reverse-geocoded to pre-fill a sensible name ("Galdhøpiggen").
    ui.newMarkerAt?.let { pos ->
        val description by viewModel.pointDescription.collectAsStateWithLifecycle()
        LaunchedEffect(pos) { viewModel.describePoint(pos) }
        MarkerEditorSheet(
            position = pos,
            suggestedName = description?.title,
            suggestedSubtitle = description?.let { listOfNotNull(it.label.takeIf { l -> l != it.title }, it.subtitle.takeIf(String::isNotBlank)).joinToString(" · ") },
            onDismiss = { ui.newMarkerAt = null; viewModel.clearPointDescription() },
            onSave = { name, kind, color, notes ->
                viewModel.addMarker(name, kind, pos, color, notes)
                ui.newMarkerAt = null
                viewModel.clearPointDescription()
            },
        )
    }
    // Edit marker: opened from the detail sheet's Edit action.
    ui.editingMarker?.let { marker ->
        MarkerEditorSheet(
            position = marker.position,
            existing = marker,
            onDismiss = { ui.editingMarker = null },
            onSave = { name, kind, color, notes ->
                viewModel.updateMarker(
                    marker.copy(name = name.ifBlank { marker.name }, kind = kind, colorArgb = color, notes = notes),
                )
                ui.selectionState.clear()
                ui.editingMarker = null
            },
        )
    }
    // Delete confirmation for the selected marker.
    ui.pendingDelete?.let { marker ->
        DeleteMarkerDialog(
            markerName = marker.name,
            onConfirm = {
                viewModel.deleteMarker(marker.id)
                ui.selectionState.clear()
                ui.pendingDelete = null
            },
            onDismiss = { ui.pendingDelete = null },
        )
    }
    // Name + save the planned route as a track (mirrors the recording save dialog).
    if (ui.showRouteSave) {
        NameInputDialog(
            title = stringResource(R.string.route_save_title),
            confirmLabel = stringResource(com.sigmundgranaas.turbo.expressive.core.designsystem.R.string.ds_save),
            initial = "Route",
            onConfirm = { name ->
                routeViewModel.saveAsTrack(name)
                Toast.makeText(context, R.string.route_saved, Toast.LENGTH_SHORT).show()
                ui.showRouteSave = false
            },
            onDismiss = { ui.showRouteSave = false },
        )
    }
    // Finish a recording: name + activity-kind picker, then persist (or discard).
    if (ui.showRecSave) {
        TrackSaveDialog(
            defaultName = "Track ${Units.distance(recDistanceM, metric)}",
            canSave = recPointCount > 1,
            onSave = { name, kind -> recordingViewModel.save(name, kind) {}; ui.showRecSave = false },
            onDiscard = { recordingViewModel.discard {}; ui.showRecSave = false },
            onDismiss = { ui.showRecSave = false },
        )
    }
}
