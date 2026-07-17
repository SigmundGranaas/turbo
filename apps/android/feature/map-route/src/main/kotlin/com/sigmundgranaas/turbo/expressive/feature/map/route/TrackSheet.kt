package com.sigmundgranaas.turbo.expressive.feature.map.route

/**
 * The create-track panel behaves as a bottom sheet with a few height stops. The
 * grabber (drag handle) moves it between them. Modelled as a pure state machine so
 * the detent transition — the thing a device gesture can't be scripted against — is
 * a drivable unit, exactly as the overhaul spec's testing philosophy demands.
 */
enum class TrackDetent {
    /** Just the mode toggle + hero stat + actions — the map stays maximally visible. */
    Collapsed,

    /** The default working height: hero + surface mix + stops/round-trip controls. */
    Default,

    /** Everything, lifted higher for editing (biggest bottom inset). */
    Expanded;

    /**
     * Roughly how much of the screen height the sheet occupies at this stop. Used only
     * to size the *rail's* clearance band (see [layoutRail]); the panel itself is
     * wrap-content, so this is an upper-bound estimate, not a hard height.
     */
    val heightFraction: Float
        get() = when (this) {
            Collapsed -> 0.24f
            Default -> 0.40f
            Expanded -> 0.56f
        }
}

/** Which way the handle was dragged (screen-up grows the sheet, screen-down shrinks it). */
enum class DragDirection { Up, Down }

/**
 * The next detent when the handle is dragged [direction] from [current]. Up steps one
 * stop taller, Down one stop shorter, both clamped at the ends — so repeated drags walk
 * the sheet through every stop and stick at the extremes rather than wrapping.
 */
fun nextDetent(current: TrackDetent, direction: DragDirection): TrackDetent {
    val order = TrackDetent.entries
    val i = order.indexOf(current)
    val next = when (direction) {
        DragDirection.Up -> i + 1
        DragDirection.Down -> i - 1
    }
    return order[next.coerceIn(0, order.lastIndex)]
}

/** Map a finished vertical drag (accumulated dy in px, screen coords) to a direction, or
 *  null if the drag never cleared the [thresholdPx] slop (a tap / tiny wobble = no change). */
fun dragDirection(totalDyPx: Float, thresholdPx: Float): DragDirection? = when {
    totalDyPx <= -thresholdPx -> DragDirection.Up
    totalDyPx >= thresholdPx -> DragDirection.Down
    else -> null
}
