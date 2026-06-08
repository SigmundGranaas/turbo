package com.sigmundgranaas.turbo.expressive.feature.map

import android.widget.Toast
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DeleteSweep
import androidx.compose.material.icons.rounded.Route
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.core.geo.Units
import com.sigmundgranaas.turbo.expressive.core.map.MapEntityActionRegistry
import com.sigmundgranaas.turbo.expressive.core.map.MapEntityDetailHost
import com.sigmundgranaas.turbo.expressive.core.map.MapSelectionState
import com.sigmundgranaas.turbo.expressive.domain.BaseLayer
import com.sigmundgranaas.turbo.expressive.domain.CollectionItemType
import com.sigmundgranaas.turbo.expressive.domain.LatLng
import com.sigmundgranaas.turbo.expressive.domain.Marker
import com.sigmundgranaas.turbo.expressive.domain.OverlayId
import com.sigmundgranaas.turbo.expressive.feature.collectionpicker.CollectionPickerSheet
import com.sigmundgranaas.turbo.expressive.feature.layers.MapLayersSheet
import com.sigmundgranaas.turbo.expressive.feature.markers.MarkerEditorSheet
import com.sigmundgranaas.turbo.expressive.feature.recording.RecordingViewModel
import com.sigmundgranaas.turbo.expressive.feature.recording.TrackSaveDialog
import com.sigmundgranaas.turbo.expressive.feature.offline.OfflineViewModel
import com.sigmundgranaas.turbo.expressive.ui.components.DeleteMarkerDialog
import com.sigmundgranaas.turbo.expressive.ui.components.NameInputDialog
import com.sigmundgranaas.turbo.expressive.ui.components.TurboConfirmDialog
import com.sigmundgranaas.turbo.expressive.ui.map.MapController

/**
 * The map's overlay layer of position-independent popups: the entity detail host plus
 * every dialog / bottom sheet that renders when its trigger state is set. Hoisted out
 * of [MapScreen] so the screen body keeps the camera + content orchestration and this
 * file owns the modal wiring. State is passed as value + `on…` callbacks so the screen
 * keeps owning the `remember`s.
 */
@Suppress("LongParameterList", "LongMethod") // a cohesive modal cluster; each arg is one popup's state
@Composable
internal fun MapScreenModals(
    viewModel: MapViewModel,
    routeViewModel: RouteViewModel,
    recordingViewModel: RecordingViewModel,
    offlineViewModel: OfflineViewModel,
    selectionState: MapSelectionState,
    actionRegistry: MapEntityActionRegistry,
    controller: MapController?,
    metric: Boolean,
    routeState: RouteUiState,
    recDistanceM: Double,
    recPointCount: Int,
    baseLayer: BaseLayer,
    activeOverlays: Set<OverlayId>,
    onToggleOverlay: (OverlayId, Boolean) -> Unit,
    onOpenOffline: () -> Unit,
    openTrackTool: (TrackMode) -> Unit,
    showLayers: Boolean,
    onShowLayers: (Boolean) -> Unit,
    showFollowStopSave: Boolean,
    onShowFollowStopSave: (Boolean) -> Unit,
    confirmReplaceFollow: Boolean,
    onConfirmReplaceFollow: (Boolean) -> Unit,
    addToCollection: Marker?,
    onAddToCollection: (Marker?) -> Unit,
    newMarkerAt: LatLng?,
    onNewMarkerAt: (LatLng?) -> Unit,
    editingMarker: Marker?,
    onEditingMarker: (Marker?) -> Unit,
    pendingDelete: Marker?,
    onPendingDelete: (Marker?) -> Unit,
    showRouteSave: Boolean,
    onShowRouteSave: (Boolean) -> Unit,
    showRecSave: Boolean,
    onShowRecSave: (Boolean) -> Unit,
) {
    val context = LocalContext.current

    // ---- Selection detail host (markers, and any future entity) ----
    MapEntityDetailHost(state = selectionState, registry = actionRegistry)

    // Stopping a follow: keep it (→ Saved Tracks) or discard. Mirrors recording's
    // save-on-stop, so a route you followed can land in your history.
    if (showFollowStopSave) {
        val plan = (routeState as? RouteUiState.Following)?.plan
        TrackSaveDialog(
            defaultName = "Route ${Units.distance(plan?.distanceM ?: 0.0, metric)}",
            canSave = (plan?.geometry?.size ?: 0) > 1,
            onSave = { name, kind ->
                routeViewModel.saveAsTrack(name, kind)
                Toast.makeText(context, R.string.route_saved, Toast.LENGTH_SHORT).show()
                onShowFollowStopSave(false)
                routeViewModel.clear(); viewModel.setFollowing(false)
            },
            onDiscard = { onShowFollowStopSave(false); routeViewModel.clear(); viewModel.setFollowing(false) },
            onDismiss = { onShowFollowStopSave(false) }, // keep following
        )
    }
    // Launching the build tool while following would drop the active route — confirm.
    if (confirmReplaceFollow) {
        TurboConfirmDialog(
            title = stringResource(R.string.route_replace_follow_title),
            body = stringResource(R.string.route_replace_follow_body),
            confirmLabel = stringResource(R.string.route_replace_follow_confirm),
            icon = Icons.Rounded.Route,
            onConfirm = { onConfirmReplaceFollow(false); openTrackTool(TrackMode.Route) },
            onDismiss = { onConfirmReplaceFollow(false) },
        )
    }

    // ---- Tool sheets ----
    if (showLayers) {
        MapLayersSheet(
            selected = baseLayer,
            onSelectBase = viewModel::setBaseLayer,
            onDownloadArea = {
                controller?.let { ctrl ->
                    val bounds = ctrl.visibleBounds()
                    val centre = LatLng((bounds.north + bounds.south) / 2, (bounds.east + bounds.west) / 2)
                    offlineViewModel.download(centre, baseLayer, bounds, ctrl.zoom())
                }
                onShowLayers(false)
                onOpenOffline()
            },
            activeOverlays = activeOverlays,
            onToggleOverlay = onToggleOverlay,
            onDismiss = { onShowLayers(false) },
        )
    }
    // Add-to-collection picker for the selected marker.
    addToCollection?.let { marker ->
        CollectionPickerSheet(
            itemId = marker.id,
            type = CollectionItemType.Marker,
            onDismiss = { onAddToCollection(null) },
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
            onDismiss = { onNewMarkerAt(null); viewModel.clearPointDescription() },
            onSave = { name, kind, color, notes ->
                viewModel.addMarker(name, kind, pos, color, notes)
                onNewMarkerAt(null)
                viewModel.clearPointDescription()
            },
        )
    }
    // Edit marker: opened from the detail sheet's Edit action.
    editingMarker?.let { marker ->
        MarkerEditorSheet(
            position = marker.position,
            existing = marker,
            onDismiss = { onEditingMarker(null) },
            onSave = { name, kind, color, notes ->
                viewModel.updateMarker(
                    marker.copy(name = name.ifBlank { marker.name }, kind = kind, colorArgb = color, notes = notes),
                )
                selectionState.clear()
                onEditingMarker(null)
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
                onPendingDelete(null)
            },
            onDismiss = { onPendingDelete(null) },
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
                onShowRouteSave(false)
            },
            onDismiss = { onShowRouteSave(false) },
        )
    }
    // Finish a recording: name + activity-kind picker, then persist (or discard).
    if (showRecSave) {
        TrackSaveDialog(
            defaultName = "Track ${Units.distance(recDistanceM, metric)}",
            canSave = recPointCount > 1,
            onSave = { name, kind -> recordingViewModel.save(name, kind) {}; onShowRecSave(false) },
            onDiscard = { recordingViewModel.discard {}; onShowRecSave(false) },
            onDismiss = { onShowRecSave(false) },
        )
    }
}
