import type { LatLng } from '../../geo';
import { reverseGeocode } from '../markers';
import { gridKey } from './stops';

/** Reverse-geocode function shape a name resolver is built over. */
export type Geocode = (lat: number, lng: number) => Promise<string>;

/** A stop's resolved name, or undefined until it's been fetched. */
export interface StopNames {
  cached(point: LatLng): string | undefined;
  resolve(point: LatLng): Promise<string | undefined>;
}

/** Lazily reverse-geocodes route stops and caches the resulting name per ~11 m
 *  grid cell, so a stop's name is fetched at most once and then re-used across
 *  re-renders, re-solves and reorders. Never throws and never blocks the solve:
 *  an offline / failed / empty lookup yields undefined and caches nothing, so a
 *  later online attempt can still succeed — the row falls back to trimmed coords.
 *  Web mirror of Android's `StopNames`. */
export function createStopNames(geocode: Geocode): StopNames {
  const cache = new Map<string, string>();

  return {
    cached: (point) => cache.get(gridKey(point)),
    resolve: async (point) => {
      const key = gridKey(point);
      const hit = cache.get(key);
      if (hit) return hit;
      let name: string | undefined;
      try {
        name = (await geocode(point.lat, point.lng))?.trim() || undefined;
      } catch {
        name = undefined;
      }
      if (name) cache.set(key, name);
      return name;
    },
  };
}

/** The app-wide stop-name resolver (single cache for the session), backed by the
 *  public reverse-geocode endpoint. Tests build their own via [createStopNames]
 *  with a fake geocoder. */
export const stopNames = createStopNames(reverseGeocode);
