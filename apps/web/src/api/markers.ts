import { ApiError, apiFetch } from './client';

/** A map marker / POI. Mirrors the geo `Location` aggregate
 *  (`/api/geo/Location`): name/description/icon + a point + a version for
 *  optimistic concurrency. There is no server-side colour — the pin tint is
 *  derived from the activity kind (icon). See port doc 06. */
export interface Marker {
  id: string;
  lat: number;
  lng: number;
  name: string;
  description: string;
  icon: string;
  version: number;
}

interface LocationResponse {
  id: string;
  geometry: { longitude: number; latitude: number };
  display: { name: string; description?: string; icon?: string };
  version: number;
}

const fromApi = (r: LocationResponse): Marker => ({
  id: r.id,
  lat: r.geometry.latitude,
  lng: r.geometry.longitude,
  name: r.display.name,
  description: r.display.description ?? '',
  icon: r.display.icon ?? '',
  version: r.version,
});

export interface MarkerInput {
  lat: number;
  lng: number;
  name: string;
  icon: string;
  description?: string;
}

/** All of the signed-in user's markers. Returns `[]` when unauthenticated
 *  (signed-out web is read-only, not an error). */
export async function listMarkers(): Promise<Marker[]> {
  try {
    const r = await apiFetch<{ items: LocationResponse[] }>('/api/geo/Location');
    return (r.items ?? []).map(fromApi);
  } catch (e) {
    if (e instanceof ApiError && (e.status === 401 || e.status === 403)) return [];
    throw e;
  }
}

export async function createMarker(input: MarkerInput): Promise<Marker> {
  const r = await apiFetch<LocationResponse>('/api/geo/Location', {
    method: 'POST',
    body: JSON.stringify({
      display: { name: input.name, description: input.description ?? '', icon: input.icon },
      geometry: { longitude: input.lng, latitude: input.lat },
    }),
  });
  return fromApi(r);
}

export async function updateMarker(m: Marker): Promise<Marker> {
  const r = await apiFetch<LocationResponse>(`/api/geo/Location/${m.id}`, {
    method: 'PUT',
    headers: { 'If-Match': String(m.version) },
    body: JSON.stringify({
      geometry: { longitude: m.lng, latitude: m.lat },
      display: { name: m.name, description: m.description, icon: m.icon },
    }),
  });
  return fromApi(r);
}

export async function deleteMarker(m: Marker): Promise<void> {
  await apiFetch(`/api/geo/Location/${m.id}`, { method: 'DELETE', headers: { 'If-Match': String(m.version) } });
}

/** Best-effort reverse geocode for pre-filling a new marker's name. Public
 *  endpoint; tolerant of the response shape (falls back to ''). */
export async function reverseGeocode(lat: number, lng: number): Promise<string> {
  try {
    const r = await apiFetch<{ name?: string; displayName?: string; placeName?: string }>(
      `/api/places/reverse?lat=${lat}&lon=${lng}`,
    );
    return r?.name ?? r?.displayName ?? r?.placeName ?? '';
  } catch {
    return '';
  }
}
