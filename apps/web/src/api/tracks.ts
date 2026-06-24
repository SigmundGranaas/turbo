import { ApiError, apiFetch } from './client';
import type { LatLng } from '../geo';

/** A saved path / track. Mirrors the tracks aggregate (`/api/tracks/Tracks`):
 *  geometry (points + optional elevations), display metadata, and stats. See
 *  port doc 08. (There is no server-side "source" field — the Flutter
 *  Recorded/Route/Drawn label isn't persisted in this API.) */
export interface Track {
  id: string;
  name: string;
  description?: string;
  colorHex?: string;
  iconKey?: string;
  points: LatLng[];
  elevations?: number[];
  distanceM: number;
  ascentM?: number;
  descentM?: number;
  movingTimeS?: number;
  recordedAt?: string;
  version: number;
}

interface TrackResponse {
  id: string;
  geometry: { points: { longitude: number; latitude: number }[]; elevations?: number[] };
  metadata: { name: string; description?: string; colorHex?: string; iconKey?: string };
  stats: { distanceMeters: number; ascentMeters?: number; descentMeters?: number; movingTimeSeconds?: number; recordedAt?: string };
  version?: number;
}

const fromApi = (r: TrackResponse): Track => ({
  id: r.id,
  name: r.metadata.name,
  description: r.metadata.description,
  colorHex: r.metadata.colorHex,
  iconKey: r.metadata.iconKey,
  points: r.geometry.points.map((p) => ({ lat: p.latitude, lng: p.longitude })),
  elevations: r.geometry.elevations,
  distanceM: r.stats.distanceMeters,
  ascentM: r.stats.ascentMeters,
  descentM: r.stats.descentMeters,
  movingTimeS: r.stats.movingTimeSeconds,
  recordedAt: r.stats.recordedAt,
  version: r.version ?? 1,
});

export interface TrackInput {
  name: string;
  points: LatLng[];
  elevations?: number[];
  iconKey?: string;
  description?: string;
  distanceM: number;
  ascentM?: number;
  movingTimeS?: number;
}

function toBody(t: TrackInput) {
  return {
    geometry: { points: t.points.map((p) => ({ longitude: p.lng, latitude: p.lat })), elevations: t.elevations },
    metadata: { name: t.name, description: t.description ?? '', iconKey: t.iconKey, smoothing: false },
    stats: {
      distanceMeters: t.distanceM,
      ascentMeters: t.ascentM,
      movingTimeSeconds: t.movingTimeS,
      recordedAt: new Date().toISOString(),
    },
  };
}

/** All of the signed-in user's tracks; `[]` when unauthenticated. */
export async function listTracks(): Promise<Track[]> {
  try {
    const r = await apiFetch<{ items: TrackResponse[] }>('/api/tracks/Tracks');
    return (r.items ?? []).map(fromApi);
  } catch (e) {
    if (e instanceof ApiError && (e.status === 401 || e.status === 403)) return [];
    throw e;
  }
}

export async function createTrack(input: TrackInput): Promise<Track> {
  const r = await apiFetch<TrackResponse>('/api/tracks/Tracks', { method: 'POST', body: JSON.stringify(toBody(input)) });
  return fromApi(r);
}

export async function deleteTrack(t: Track): Promise<void> {
  await apiFetch(`/api/tracks/Tracks/${t.id}`, { method: 'DELETE', headers: { 'If-Match': String(t.version) } });
}

/** Serialise a track to a GPX 1.1 document for download/export. */
export function toGpx(t: Track): string {
  const esc = (s: string) => s.replace(/[<>&]/g, (c) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;' })[c] ?? c);
  const seg = t.points
    .map((p, i) => {
      const ele = t.elevations?.[i];
      return `<trkpt lat="${p.lat}" lon="${p.lng}">${ele != null ? `<ele>${ele}</ele>` : ''}</trkpt>`;
    })
    .join('');
  return `<?xml version="1.0" encoding="UTF-8"?>\n<gpx version="1.1" creator="Turbo" xmlns="http://www.topografix.com/GPX/1/1"><trk><name>${esc(
    t.name,
  )}</name><trkseg>${seg}</trkseg></trk></gpx>`;
}
