/** Weather-pin seams (map-overhaul Phase 3) — mirrors the native Android
 *  `WeatherPin` work. A weather pin is NOT a new entity: it's the SAME
 *  persisted marker, distinguished only by a `markerKind` discriminator, with
 *  the last MET forecast cached on the node so it renders instantly (and
 *  offline-safe, "updated Nh ago") and only refetches when stale + online.
 *
 *  Everything a user-visible behaviour turns on lives here as a PURE function
 *  the tests drive directly (the on-map glyph / tap / refresh above is thin
 *  React wiring around these seams). Kept out of `api.ts` so the model types
 *  and the wire-icon codec can be imported without pulling in the API client. */

/** A marker is either an ordinary POI pin or a live weather pin. Persisted (see
 *  the wire-icon codec below — the geo `location` has no kind field of its own). */
export type MarkerKind = 'Standard' | 'WeatherPin';

/** The last forecast cached onto a weather-pin marker. Air fields are always
 *  present; the marine fields are only set where the point is at sea (they
 *  drive the "expanded" ocean row and `hasMarine`). */
export interface WeatherSnapshot {
  temperatureC: number;
  symbolCode?: string;
  windSpeedMs: number;
  windFromDeg: number;
  precipitationMm: number | null;
  waveHeightM?: number | null;
  waveFromDeg?: number | null;
  seaTemperatureC?: number | null;
}

/** A cached forecast counts as stale after an hour — past that (and online) a
 *  weather pin refetches on open/drop; within it, the cache is shown as-is. */
export const WEATHER_PIN_STALE_MS = 3_600_000;

/** Whether an open/dropped weather pin should hit MET or render from cache.
 *  Offline always uses whatever cache exists (a fetch can't succeed anyway); a
 *  pin with no cache, or a cache older than `staleAfterMs`, refetches. */
export function weatherPinFetchDecision(
  cacheAgeMs: number | null,
  online: boolean,
  staleAfterMs: number = WEATHER_PIN_STALE_MS,
): 'Fetch' | 'UseCache' {
  if (!online) return 'UseCache';
  if (cacheAgeMs == null) return 'Fetch';
  if (cacheAgeMs > staleAfterMs) return 'Fetch';
  return 'UseCache';
}

/** What a weather-pin marker carries for rendering — a minimal shape so the
 *  seam doesn't depend on the full `Marker`. */
export interface ForecastCarrier {
  forecast?: WeatherSnapshot;
  forecastFetchedAtEpochMs?: number;
}

/** The render state of a weather pin's on-map glyph + expanded card, derived
 *  purely from its cached forecast. `null` ONLY when there's no cached forecast
 *  yet (nothing to draw — the pin shows a spinner/placeholder instead).
 *
 *  `updatedHoursAgo` = whole hours since the cache was written (`null` when no
 *  timestamp); `hasMarine` is true when wave/sea-temperature data is present. */
export interface WeatherPinUiState {
  temperatureC: number;
  symbolCode?: string;
  updatedHoursAgo: number | null;
  hasMarine: boolean;
  expanded: {
    windSpeedMs: number;
    windFromDeg: number;
    precipitationMm: number | null;
    waveHeightM?: number | null;
    waveFromDeg?: number | null;
    seaTemperatureC?: number | null;
  };
}

export function weatherPinUiState(marker: ForecastCarrier, nowMs: number): WeatherPinUiState | null {
  const f = marker.forecast;
  if (!f) return null;
  const fetchedAt = marker.forecastFetchedAtEpochMs;
  const updatedHoursAgo =
    fetchedAt == null ? null : Math.floor((nowMs - fetchedAt) / WEATHER_PIN_STALE_MS);
  const hasMarine = f.waveHeightM != null || f.seaTemperatureC != null;
  return {
    temperatureC: f.temperatureC,
    symbolCode: f.symbolCode,
    updatedHoursAgo,
    hasMarine,
    expanded: {
      windSpeedMs: f.windSpeedMs,
      windFromDeg: f.windFromDeg,
      precipitationMm: f.precipitationMm,
      waveHeightM: f.waveHeightM,
      waveFromDeg: f.waveFromDeg,
      seaTemperatureC: f.seaTemperatureC,
    },
  };
}

// ── Marker-kind on the sync wire ────────────────────────────────────────────
// The geo `location` aggregate has no marker-kind field — only free-form
// name/description/icon. So the kind is namespaced into `icon`: a WeatherPin's
// activity icon is prefixed, and everything else round-trips as Standard. This
// is the latent data-loss fix — without it a WeatherPin comes back Standard
// after a sync round-trip.

export const WEATHER_PIN_ICON_PREFIX = 'weatherpin:';

/** Encode `(activityIcon, kind)` into the single wire `icon` string. */
export function encodeWireIcon(activityIcon: string, markerKind: MarkerKind): string {
  return markerKind === 'WeatherPin' ? `${WEATHER_PIN_ICON_PREFIX}${activityIcon}` : activityIcon;
}

/** Decode the wire `icon` back into `(activityIcon, kind)`. A `weatherpin:`
 *  prefix marks a WeatherPin (and is stripped); a blank icon defaults to a
 *  Standard `mountain` pin (mirrors the native default). */
export function decodeWireIcon(wireIcon: string): { activityIcon: string; markerKind: MarkerKind } {
  if (wireIcon.startsWith(WEATHER_PIN_ICON_PREFIX)) {
    return { activityIcon: wireIcon.slice(WEATHER_PIN_ICON_PREFIX.length), markerKind: 'WeatherPin' };
  }
  return { activityIcon: wireIcon || 'mountain', markerKind: 'Standard' };
}

/** Build a cached snapshot from the shared conditions payload (the refresh
 *  wiring calls this before persisting onto the marker). Marine fields come
 *  from the ocean data where present. */
export function snapshotFromConditions(now: {
  tempC: number;
  windMs: number;
  windDeg: number;
  precipMm: number | null;
  symbol?: string;
}, marine?: { waveHeightM?: number | null; waveFromDeg?: number | null; seaTemperatureC?: number | null }): WeatherSnapshot {
  return {
    temperatureC: now.tempC,
    symbolCode: now.symbol,
    windSpeedMs: now.windMs,
    windFromDeg: now.windDeg,
    precipitationMm: now.precipMm,
    waveHeightM: marine?.waveHeightM ?? null,
    waveFromDeg: marine?.waveFromDeg ?? null,
    seaTemperatureC: marine?.seaTemperatureC ?? null,
  };
}
