package com.sigmundgranaas.turbo.expressive.core.map

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.domain.LatLng

/**
 * The single "what is the user inspecting on the map" model. Any surface
 * (marker tap, search result, long-press coordinate, activity link) sets a
 * [MapSelection]; one detail host renders it — instead of each feature opening
 * its own sheet. Mirrors the Flutter selection-model + detail-host seam.
 */
data class MapSelection(
    val title: String,
    val subtitle: String? = null,
    /** Entity id (e.g. marker id) so the map can highlight the selected pin. */
    val id: String? = null,
    val point: LatLng? = null,
    val path: GeoPath? = null,
    val extraActions: List<MapEntityAction> = emptyList(),
    val includeStandardActions: Boolean = true,
    val onNavigate: (() -> Unit)? = null,
    val onShare: (() -> Unit)? = null,
    val onEdit: (() -> Unit)? = null,
    val onDelete: (() -> Unit)? = null,
    val body: (@Composable () -> Unit)? = null,
) {
    fun toActionContext(): MapEntityActionContext = MapEntityActionContext(
        title = title,
        point = point,
        path = path,
        onNavigate = onNavigate,
        onShare = onShare,
        onEdit = onEdit,
        onDelete = onDelete,
    )
}

/** Holds the current selection. Hoist one per map shell. */
class MapSelectionState {
    var selection: MapSelection? by mutableStateOf(null)
        private set

    fun select(value: MapSelection) { selection = value }
    fun clear() { selection = null }
}
