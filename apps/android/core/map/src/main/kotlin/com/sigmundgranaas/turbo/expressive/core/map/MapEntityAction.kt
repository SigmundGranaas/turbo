package com.sigmundgranaas.turbo.expressive.core.map

import androidx.compose.ui.graphics.vector.ImageVector
import com.sigmundgranaas.turbo.expressive.core.geo.GeoPath
import com.sigmundgranaas.turbo.expressive.domain.LatLng

/**
 * What a selected map entity (marker, path, activity, coordinate) can offer.
 * Standard actions (Navigate/Share/Edit/Delete) and feature-contributed
 * `extraActions` flow through the same [MapEntityActionRegistry] — mirrors the
 * Flutter app's action-bar seam. Gate availability with [isAvailable].
 */
data class MapEntityActionContext(
    val title: String,
    val point: LatLng? = null,
    val path: GeoPath? = null,
    val onNavigate: (() -> Unit)? = null,
    val onShare: (() -> Unit)? = null,
    val onEdit: (() -> Unit)? = null,
    val onDelete: (() -> Unit)? = null,
)

data class MapEntityAction(
    val id: String,
    val label: String,
    val icon: ImageVector,
    val priority: Int = 0,
    val isDestructive: Boolean = false,
    val isAvailable: (MapEntityActionContext) -> Boolean = { true },
    val onInvoke: (MapEntityActionContext) -> Unit,
)

/**
 * Merges standard actions with an entity's [MapEntityActionContext]-contributed
 * extras and returns those available, highest [MapEntityAction.priority] first.
 */
class MapEntityActionRegistry(private val standard: List<MapEntityAction>) {
    fun availableFor(
        ctx: MapEntityActionContext,
        extraActions: List<MapEntityAction> = emptyList(),
        includeStandard: Boolean = true,
    ): List<MapEntityAction> =
        ((if (includeStandard) standard else emptyList()) + extraActions)
            .filter { it.isAvailable(ctx) }
            .sortedByDescending { it.priority }
}
