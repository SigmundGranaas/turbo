import type { LatLng } from '../geo';

/**
 * The unified "map point card" state — the quick-actions menu the map shows for
 * a point (a weather header + Add Marker / Route Here / Measure, where Add
 * Marker expands to Marker / Photo / Weather Pin).
 *
 * A faithful port of Android's `MapPointCard` reducer (spec Phase 2): pure so the
 * tap semantics are testable without React or a real pointer. A tap on empty map
 * opens it, a tap on an entity lets the entity win (card stays hidden), a tap
 * while open RE-ANCHORS it to the new point (collapsing any expansion) rather
 * than dismissing, a long-press opens it even over an entity (drop-on-top), a
 * pan dismisses it, and track/route mode suppresses it entirely (taps place
 * points instead). See docs/architecture/2026-07-turbo-map-overhaul-spec.md.
 */
export type MapPointCard =
  | { readonly kind: 'hidden' }
  /** Shown at [point]; [expanded] reveals the Marker / Photo / Weather-Pin sub-row. */
  | { readonly kind: 'shown'; readonly point: LatLng; readonly expanded: boolean };

export const HIDDEN: MapPointCard = { kind: 'hidden' };

export const shownCard = (point: LatLng, expanded = false): MapPointCard => ({
  kind: 'shown',
  point,
  expanded,
});

/** Inputs the card reduces over. */
export type MapPointCardEvent =
  /** A tap landed at [point]; [onEntity] = it hit a marker/pin/POI (the entity wins). */
  | { readonly type: 'tap'; readonly point: LatLng; readonly onEntity: boolean }
  /** A long-press at [point] — opens the card even over an entity. */
  | { readonly type: 'long-press'; readonly point: LatLng }
  /** The user started moving the camera — the card dismisses. */
  | { readonly type: 'pan' }
  /** Explicit dismiss (close affordance / Escape). */
  | { readonly type: 'dismiss' }
  /** Toggle the Add-Marker expansion (no-op while hidden). */
  | { readonly type: 'toggle-add-marker' };

/**
 * The card's transition function. `trackModeActive` suppresses the card entirely
 * (in route/line/draw mode a tap places a point; the card never opens).
 */
export function reduceMapPointCard(
  state: MapPointCard,
  event: MapPointCardEvent,
  trackModeActive: boolean,
): MapPointCard {
  switch (event.type) {
    case 'tap':
      // Track mode owns taps (they place/extend points) — never open the card.
      if (trackModeActive) return HIDDEN;
      // Tapping an entity opens THAT entity's detail elsewhere; the card yields.
      if (event.onEntity) return HIDDEN;
      // Empty tap opens the card, or re-anchors an open one to the new point
      // (collapsing any expansion) — a one-gesture reposition, not a dismiss.
      return shownCard(event.point, false);
    case 'long-press':
      // Long-press opens even over an entity (drop-on-top); track mode still wins.
      return trackModeActive ? HIDDEN : shownCard(event.point, false);
    case 'pan':
    case 'dismiss':
      return HIDDEN;
    case 'toggle-add-marker':
      return state.kind === 'shown' ? shownCard(state.point, !state.expanded) : HIDDEN;
  }
}

/** The Measure action's availability given connectivity — measuring needs the
 *  routing/elevation services, so it's offline-disabled with a hint. Pure so the
 *  gate is testable without the network. */
export interface MeasureAvailability {
  readonly enabled: boolean;
  readonly hint?: string;
}

export function measureAvailability(online: boolean): MeasureAvailability {
  return online ? { enabled: true } : { enabled: false, hint: 'Measure needs a connection' };
}
