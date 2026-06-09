package com.sigmundgranaas.turbo.expressive.core.map

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.IosShare
import androidx.compose.material.icons.rounded.Navigation

/**
 * The standard action set every selected entity offers when its context
 * supplies the relevant capability/callback. Features add their own via
 * `extraActions` on the [MapSelection]; the registry merges them.
 */
fun defaultMapEntityActions(): List<MapEntityAction> = listOf(
    MapEntityAction(
        id = "navigate",
        label = "Navigate",
        icon = Icons.Rounded.Navigation,
        priority = 100,
        isAvailable = { it.point != null || it.path != null },
        onInvoke = { it.onNavigate?.invoke() },
    ),
    // Edit outranks Share so it becomes the prominent square "quick" action in the
    // detail bar (Share folds into the overflow menu).
    MapEntityAction(
        id = "edit",
        label = "Edit",
        icon = Icons.Rounded.Edit,
        priority = 80,
        isAvailable = { it.onEdit != null },
        onInvoke = { it.onEdit?.invoke() },
    ),
    MapEntityAction(
        id = "share",
        label = "Share",
        icon = Icons.Rounded.IosShare,
        priority = 60,
        isAvailable = { it.onShare != null },
        onInvoke = { it.onShare?.invoke() },
    ),
    MapEntityAction(
        id = "delete",
        label = "Delete",
        icon = Icons.Rounded.Delete,
        priority = 40,
        isDestructive = true,
        isAvailable = { it.onDelete != null },
        onInvoke = { it.onDelete?.invoke() },
    ),
)

/** A registry pre-loaded with [defaultMapEntityActions]. */
fun defaultMapEntityActionRegistry(): MapEntityActionRegistry =
    MapEntityActionRegistry(defaultMapEntityActions())
