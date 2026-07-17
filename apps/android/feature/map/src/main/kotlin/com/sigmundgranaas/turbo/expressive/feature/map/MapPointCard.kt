package com.sigmundgranaas.turbo.expressive.feature.map

import com.sigmundgranaas.turbo.expressive.domain.LatLng

/**
 * The quick-actions "map point card" state — the unified menu the map shows for
 * a point (weather header + Add Marker / Route Here / Measure, with the Add
 * Marker row expanding to Marker / Photo / Weather Pin).
 *
 * Pure so the tap semantics are testable without Compose: a tap on empty map
 * opens it, a tap on an entity lets the entity win (card stays hidden), a tap
 * while open RE-ANCHORS it (collapsing any expansion) rather than dismissing,
 * a long-press opens it even over an entity (drop-on-top), and anything in
 * track/route mode is suppressed (taps place points instead). See
 * docs/architecture/2026-07-turbo-map-overhaul-spec.md (Phase 2).
 */
sealed interface MapPointCard {
    data object Hidden : MapPointCard

    /** Shown at [point]; [expanded] reveals the Marker/Photo/Weather-Pin sub-row. */
    data class Shown(val point: LatLng, val expanded: Boolean = false) : MapPointCard
}

/** Inputs the card reduces over. */
sealed interface MapPointCardEvent {
    /** A tap landed at [point]; [onEntity] = it hit a marker/pin/POI (entity wins). */
    data class Tap(val point: LatLng, val onEntity: Boolean) : MapPointCardEvent

    /** A long-press at [point] — opens the card even over an entity. */
    data class LongPress(val point: LatLng) : MapPointCardEvent

    /** The user started moving the camera — the card dismisses. */
    data object Pan : MapPointCardEvent

    /** Explicit dismiss (close affordance / back). */
    data object Dismiss : MapPointCardEvent

    /** Toggle the Add-Marker expansion (no-op while hidden). */
    data object ToggleAddMarker : MapPointCardEvent
}

/**
 * The card's transition function. [trackModeActive] suppresses the card entirely
 * (in Route/Line/Draw mode a tap places a point; the card never opens).
 */
fun reduceMapPointCard(
    state: MapPointCard,
    event: MapPointCardEvent,
    trackModeActive: Boolean,
): MapPointCard = when (event) {
    is MapPointCardEvent.Tap -> when {
        // Track mode owns taps (they place/extend points) — never open the card.
        trackModeActive -> MapPointCard.Hidden
        // Tapping an entity opens THAT entity's detail elsewhere; the card yields.
        event.onEntity -> MapPointCard.Hidden
        // Empty tap opens the card, or re-anchors an open one to the new point
        // (collapsing any expansion) — a one-gesture reposition, not a dismiss.
        else -> MapPointCard.Shown(event.point, expanded = false)
    }
    is MapPointCardEvent.LongPress ->
        // Long-press opens the card even over an entity (drop-on-top) — but track
        // mode still suppresses it.
        if (trackModeActive) MapPointCard.Hidden else MapPointCard.Shown(event.point, expanded = false)
    MapPointCardEvent.Pan -> MapPointCard.Hidden
    MapPointCardEvent.Dismiss -> MapPointCard.Hidden
    MapPointCardEvent.ToggleAddMarker -> when (state) {
        is MapPointCard.Shown -> state.copy(expanded = !state.expanded)
        MapPointCard.Hidden -> MapPointCard.Hidden
    }
}
