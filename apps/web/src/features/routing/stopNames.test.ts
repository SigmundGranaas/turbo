import { describe, it, expect, vi } from 'vitest';
import type { LatLng } from '../../geo';
import { createStopNames } from './stopNames';

const at = (lat: number, lng: number): LatLng => ({ lat, lng });

/** Lazy, cached reverse-geocoding of route stops: a name is fetched at most once
 *  per ~11 m grid cell, survives a reorder (it's keyed on the coordinate), and a
 *  failed/empty lookup never throws — the caller falls back to trimmed coords. */
describe('stop name resolution', () => {
  it('resolves a stop to its geocoded name', async () => {
    const names = createStopNames(async () => 'Storgata 1');
    expect(await names.resolve(at(69.96, 23.27))).toBe('Storgata 1');
  });

  it('geocodes a coordinate only once, then serves the cache on a re-render', async () => {
    const geocode = vi.fn(async () => 'Storgata 1');
    const names = createStopNames(geocode);
    const p = at(69.96, 23.27);
    await names.resolve(p);
    await names.resolve(p);
    expect(geocode).toHaveBeenCalledTimes(1);
    expect(names.cached(p)).toBe('Storgata 1');
  });

  it('keeps the name for a stop after a reorder moves it (same coordinate, later query)', async () => {
    const geocode = vi.fn(async () => 'Fjelltoppen');
    const names = createStopNames(geocode);
    const p = at(69.9607, 23.2715);
    await names.resolve(p); // resolved while it was, say, the 2nd stop
    // after a reorder the row for the same place asks again — served from cache,
    // even for a coordinate a few metres off inside the same ~11 m cell
    expect(names.cached(at(69.96072, 23.27152))).toBe('Fjelltoppen');
    expect(geocode).toHaveBeenCalledTimes(1);
  });

  it('yields undefined and never throws when the lookup fails (offline)', async () => {
    const names = createStopNames(async () => {
      throw new Error('offline');
    });
    await expect(names.resolve(at(69.96, 23.27))).resolves.toBeUndefined();
  });

  it('does not cache an empty result, so a later online attempt can still succeed', async () => {
    let online = false;
    const names = createStopNames(async () => (online ? 'Storgata 1' : ''));
    const p = at(69.96, 23.27);
    expect(await names.resolve(p)).toBeUndefined();
    online = true;
    expect(await names.resolve(p)).toBe('Storgata 1');
  });
});
