import { API_BASE } from '../../config';
import type { LatLng } from '../../geo';

/** Trip-style presets (match the tileserver pathfinder). */
export const ROUTE_PRESETS = [
  { key: 'balanced', label: 'Balanced', icon: 'tune' },
  { key: 'avoid_roads', label: 'Avoid roads', icon: 'forest' },
  { key: 'trail_purist', label: 'Marked trails', icon: 'route' },
  { key: 'easy_grade', label: 'Easy grade', icon: 'trending_down' },
  { key: 'direct', label: 'Direct', icon: 'straight' },
] as const;
export type RoutePresetKey = (typeof ROUTE_PRESETS)[number]['key'];

/** Movement profiles → the design's mode tabs. */
export const ROUTE_PROFILES = [
  { key: 'foot', label: 'Hike', icon: 'hiking' },
  { key: 'ski', label: 'Ski', icon: 'downhill_skiing' },
  { key: 'bicycle', label: 'Bike', icon: 'directions_bike' },
] as const;
export type RouteProfile = (typeof ROUTE_PROFILES)[number]['key'];

export interface RoutePlan {
  distanceM: number;
  durationS: number;
  ascentM: number;
  onTrailPct: number;
  coords: LatLng[];
}

export interface PlanCallbacks {
  onProgress?: (coords: LatLng[]) => void;
  onResult?: (plan: RoutePlan) => void;
  onError?: (message: string) => void;
}

const toLatLng = (pair: [number, number]): LatLng => ({ lng: pair[0], lat: pair[1] });

/** Stream a route from the public SSE pathfinder. Reads the `progress` (live
 *  best-path snapshots) → `result` (final plan) / `error` events off a `fetch`
 *  ReadableStream (EventSource can't POST). Mirrors Android's `RouteSse`. */
export async function planStream(
  points: LatLng[],
  preset: RoutePresetKey,
  profile: RouteProfile,
  cb: PlanCallbacks,
  signal?: AbortSignal,
): Promise<void> {
  const res = await fetch(`${API_BASE}/api/route/plan/stream`, {
    method: 'POST',
    // The routing API is public and answers with `Access-Control-Allow-Origin: *`.
    // A browser REFUSES a credentialed (`include`) request against a wildcard CORS
    // origin, so the stream silently never arrives — omit credentials (no auth
    // needed) to let the wildcard response through. (curl ignores CORS, which hid
    // this.)
    credentials: 'omit',
    headers: { 'Content-Type': 'application/json', Accept: 'text/event-stream' },
    body: JSON.stringify({ points: points.map((p) => [p.lng, p.lat]), preset, profile }),
    signal,
  });
  if (!res.ok || !res.body) {
    cb.onError?.(`Routing failed (${res.status})`);
    return;
  }
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = '';
  let event: string | null = null;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });
    let nl: number;
    while ((nl = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, nl).replace(/\r$/, '');
      buf = buf.slice(nl + 1);
      if (line === '') {
        event = null;
      } else if (line.startsWith('event:')) {
        event = line.slice(6).trim();
      } else if (line.startsWith('data:')) {
        handle(event, line.slice(5).trim(), cb);
      }
    }
  }
}

function handle(event: string | null, data: string, cb: PlanCallbacks) {
  let json: unknown;
  try {
    json = JSON.parse(data);
  } catch {
    return;
  }
  if (event === 'progress') {
    const coords = (json as { coordinates?: [number, number][] }).coordinates ?? [];
    cb.onProgress?.(coords.map(toLatLng));
  } else if (event === 'result') {
    const r = json as {
      distance_m?: number;
      duration_s?: number;
      ascent_m?: number;
      on_trail_pct?: number;
      geometry?: { coordinates?: [number, number][] };
    };
    cb.onResult?.({
      distanceM: r.distance_m ?? 0,
      durationS: r.duration_s ?? 0,
      ascentM: r.ascent_m ?? 0,
      onTrailPct: r.on_trail_pct ?? 0,
      coords: (r.geometry?.coordinates ?? []).map(toLatLng),
    });
  } else if (event === 'error') {
    cb.onError?.((json as { error?: string }).error ?? 'Routing failed');
  }
}
