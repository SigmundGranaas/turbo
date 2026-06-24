import { apiFetch } from './client';

/** Weather at a point — the subset the UI renders. Backed by the public
 *  `/api/activities/conditions` proxy (server-side met.no; the browser can't
 *  call api.met.no directly). See port doc 13. */
export interface Weather {
  tempC: number;
  windMs: number;
  windDeg: number;
  precipMm: number | null;
  humidityPct: number;
  cloudPct: number;
  symbol?: string;
}

interface WeatherSlice {
  airTemperatureCelsius: number;
  windSpeedMs: number;
  windFromDegrees: number;
  precipitationNext1hMm: number | null;
  relativeHumidityPct: number;
  cloudCoveragePct: number;
  symbolCode?: string;
}

const mapSlice = (s: WeatherSlice): Weather => ({
  tempC: s.airTemperatureCelsius,
  windMs: s.windSpeedMs,
  windDeg: s.windFromDegrees,
  precipMm: s.precipitationNext1hMm,
  humidityPct: s.relativeHumidityPct,
  cloudPct: s.cloudCoveragePct,
  symbol: s.symbolCode,
});

export interface Tide {
  heightM: number | null;
  summary: string | null;
}

export interface Conditions {
  now: Weather;
  hourly: Weather[];
  tide?: Tide;
}

export async function getConditions(lat: number, lng: number): Promise<Conditions> {
  const r = await apiFetch<{
    now: WeatherSlice;
    hourly: WeatherSlice[];
    tide?: { currentHeightMeters: number | null; summary: string | null };
  }>(`/api/activities/conditions?lat=${lat.toFixed(4)}&lon=${lng.toFixed(4)}`);
  return {
    now: mapSlice(r.now),
    hourly: (r.hourly ?? []).map(mapSlice),
    tide: r.tide ? { heightM: r.tide.currentHeightMeters, summary: r.tide.summary } : undefined,
  };
}

/** 8-point compass label for a wind-from bearing. */
export function windDir(deg: number): string {
  return ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'][Math.round(deg / 45) % 8];
}
