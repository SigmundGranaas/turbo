import { useEffect, useRef } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { getConditions } from '../../api/conditions';
import type { Marker } from './api';
import { snapshotFromConditions, weatherPinFetchDecision } from './weatherPin';

const KEY = ['markers'];

/** Keep every weather pin's cached forecast live — the web mirror of Android's
 *  `WeatherPinViewModel.refreshPins`. For each weather-pin marker it applies the
 *  pure fetch decision (only pins whose cache is absent/stale, and only when
 *  online), dedupes in-flight fetches per pin, and persists the fresh snapshot
 *  back onto the marker in the query cache so the chip and detail render from it.
 *
 *  The forecast cache is client-side only (the geo `location` has no forecast
 *  field), so it's patched into the react-query cache, not written to the API. */
export function useWeatherPinForecasts(markers: Marker[]): void {
  const qc = useQueryClient();
  const inFlight = useRef(new Set<string>());

  useEffect(() => {
    const online = typeof navigator === 'undefined' ? true : navigator.onLine;
    const now = Date.now();
    for (const m of markers) {
      if (m.markerKind !== 'WeatherPin') continue;
      const ageMs = m.forecastFetchedAtEpochMs == null ? null : now - m.forecastFetchedAtEpochMs;
      if (weatherPinFetchDecision(ageMs, online) !== 'Fetch') continue;
      if (inFlight.current.has(m.id)) continue;
      inFlight.current.add(m.id);
      getConditions(m.lat, m.lng)
        .then((c) => {
          const snapshot = snapshotFromConditions(c.now);
          const fetchedAt = Date.now();
          qc.setQueryData<Marker[]>(KEY, (old) =>
            (old ?? []).map((x) =>
              x.id === m.id ? { ...x, forecast: snapshot, forecastFetchedAtEpochMs: fetchedAt } : x,
            ),
          );
        })
        .catch(() => {
          /* offline / MET hiccup — the pin keeps whatever cache it had */
        })
        .finally(() => inFlight.current.delete(m.id));
    }
  }, [markers, qc]);
}
