package com.sigmundgranaas.turbo.expressive.ui.layout

/**
 * One item in the map control rail. [essential] items (compass, layers,
 * location) never auto-hide; non-essential ones (the +/− zoom buttons) hide
 * first when vertical space is tight. Order in the input list is the visual
 * order top→bottom.
 */
data class RailItem(val id: String, val heightPx: Float, val essential: Boolean)

/** Where a rail item lands: its top edge (px) and whether it's shown at all. */
data class RailPlacement(val id: String, val topPx: Float, val visible: Boolean)

/**
 * Pure collision/auto-hide layout for the map control rail (the "smart slide"
 * from the spec). The rail lives in the vertical band `[topBoundPx, bottomBoundPx]`
 * — `topBoundPx` is the bottom of the search bar (the rail **must never overlap
 * it**), `bottomBoundPx` is the top of whatever occupies the bottom (a live
 * sheet, the nav bar). Items are bottom-anchored (buttons sit low, near the
 * thumb). When they don't all fit, **non-essential items hide first** (+/− zoom),
 * so a growing sheet sheds the zoom buttons rather than shoving essentials over
 * the search bar.
 *
 * No Compose, no measurement side effects — this is the seam the rail's
 * behaviour is tested against. See docs/architecture/2026-07-turbo-map-overhaul-spec.md.
 */
fun layoutRail(
    items: List<RailItem>,
    topBoundPx: Float,
    bottomBoundPx: Float,
    spacingPx: Float,
): List<RailPlacement> {
    val available = bottomBoundPx - topBoundPx
    // Greedily drop non-essential items (last-listed first — the +/− at the
    // bottom of the stack) until the remaining set fits the band.
    val kept = items.toMutableList()
    fun stackHeight(list: List<RailItem>): Float =
        if (list.isEmpty()) 0f else list.sumOf { it.heightPx.toDouble() }.toFloat() + spacingPx * (list.size - 1)

    while (stackHeight(kept) > available && kept.any { !it.essential }) {
        val dropIndex = kept.indexOfLast { !it.essential }
        kept.removeAt(dropIndex)
    }

    val visibleIds = kept.map { it.id }.toSet()
    val stackH = stackHeight(kept)
    // Bottom-anchor the kept stack; clamp the top so it never crosses the search
    // bar even in the degenerate case where essentials alone overflow.
    var top = (bottomBoundPx - stackH).coerceAtLeast(topBoundPx)

    val placements = LinkedHashMap<String, RailPlacement>()
    for (item in items) {
        if (item.id in visibleIds) {
            placements[item.id] = RailPlacement(item.id, top, visible = true)
            top += item.heightPx + spacingPx
        } else {
            placements[item.id] = RailPlacement(item.id, topPx = 0f, visible = false)
        }
    }
    // Preserve the caller's order.
    return items.map { placements.getValue(it.id) }
}
