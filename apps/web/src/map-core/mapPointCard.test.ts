import { describe, it, expect } from 'vitest';
import {
  reduceMapPointCard,
  measureAvailability,
  HIDDEN,
  shownCard,
  type MapPointCard,
  type MapPointCardEvent,
} from './mapPointCard';

/**
 * The map-point card's tap semantics as a user experiences them: an empty tap
 * opens it, an entity tap yields to the entity, a second tap re-anchors (not
 * dismiss), long-press drops on top of an entity, panning dismisses, and track
 * mode suppresses it entirely. Pure reducer — the whole point is testing the tap
 * behaviour without React or a device.
 */
describe('map point card reducer', () => {
  const a = { lat: 69.96, lng: 23.27 };
  const b = { lat: 69.97, lng: 23.3 };
  const reduce = (state: MapPointCard, event: MapPointCardEvent, track = false) =>
    reduceMapPointCard(state, event, track);

  it('opens the card at the point on an empty-map tap', () => {
    expect(reduce(HIDDEN, { type: 'tap', point: a, onEntity: false })).toEqual(shownCard(a, false));
  });

  it('yields to the entity on an entity tap (card stays hidden)', () => {
    expect(reduce(HIDDEN, { type: 'tap', point: a, onEntity: true })).toEqual(HIDDEN);
  });

  it('re-anchors to the new point and collapses expansion when tapped while open', () => {
    const open = shownCard(a, true);
    expect(reduce(open, { type: 'tap', point: b, onEntity: false })).toEqual(shownCard(b, false));
  });

  it('opens the card even over an entity on long-press', () => {
    expect(reduce(HIDDEN, { type: 'long-press', point: a })).toEqual(shownCard(a, false));
  });

  it('dismisses the card on a pan', () => {
    expect(reduce(shownCard(a), { type: 'pan' }, false)).toEqual(HIDDEN);
  });

  it('dismisses the card on an explicit dismiss', () => {
    expect(reduce(shownCard(a, true), { type: 'dismiss' }, false)).toEqual(HIDDEN);
  });

  it('suppresses the card entirely in track mode — taps place points instead', () => {
    expect(reduce(HIDDEN, { type: 'tap', point: a, onEntity: false }, true)).toEqual(HIDDEN);
    expect(reduce(HIDDEN, { type: 'long-press', point: a }, true)).toEqual(HIDDEN);
  });

  it('toggles the Add Marker expansion only while shown', () => {
    const open = shownCard(a, false);
    const expanded = reduce(open, { type: 'toggle-add-marker' });
    expect(expanded).toEqual(shownCard(a, true));
    expect(reduce(expanded, { type: 'toggle-add-marker' })).toEqual(shownCard(a, false));
    // No-op while hidden.
    expect(reduce(HIDDEN, { type: 'toggle-add-marker' })).toEqual(HIDDEN);
  });
});

/** Measure needs the network — it's enabled online and disabled (with a hint) offline. */
describe('measure availability gate', () => {
  it('is enabled with no hint when online', () => {
    expect(measureAvailability(true)).toEqual({ enabled: true });
  });

  it('is disabled with a connection hint when offline', () => {
    const off = measureAvailability(false);
    expect(off.enabled).toBe(false);
    expect(off.hint).toMatch(/connection/i);
  });
});
