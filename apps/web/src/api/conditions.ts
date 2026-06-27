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

/** One day's rolled-up outlook. `date` is an ISO `YYYY-MM-DD` string (UTC). */
export interface DayForecast {
  date: string;
  highC: number;
  lowC: number;
  precipMm: number | null;
  symbol?: string;
}

interface DaySlice {
  date: string;
  highC: number;
  lowC: number;
  precipMm: number | null;
  symbolCode?: string;
}

const mapDay = (d: DaySlice): DayForecast => ({
  date: d.date,
  highC: d.highC,
  lowC: d.lowC,
  precipMm: d.precipMm,
  symbol: d.symbolCode,
});

export interface Conditions {
  now: Weather;
  hourly: Weather[];
  daily: DayForecast[];
  tide?: Tide;
}

export async function getConditions(lat: number, lng: number): Promise<Conditions> {
  const r = await apiFetch<{
    now: WeatherSlice;
    hourly: WeatherSlice[];
    daily?: DaySlice[];
    tide?: { currentHeightMeters: number | null; summary: string | null };
  }>(`/api/activities/conditions?lat=${lat.toFixed(4)}&lon=${lng.toFixed(4)}`);
  return {
    now: mapSlice(r.now),
    hourly: (r.hourly ?? []).map(mapSlice),
    // `daily` is tolerated as absent so the web bundle can ship ahead of the
    // backend deploy that adds it (the "Next days" section just stays hidden).
    daily: (r.daily ?? []).map(mapDay),
    tide: r.tide ? { heightM: r.tide.currentHeightMeters, summary: r.tide.summary } : undefined,
  };
}

/** 8-point compass label for a wind-from bearing. */
export function windDir(deg: number): string {
  return ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'][Math.round(deg / 45) % 8];
}
