import { API_BASE } from '../config';
import type { LatLng } from '../geo';

/** Per-vertex terrain elevations from the tileserver DEM
 *  (`POST /v1/elev/samples`) — the elevation-backfill primitive. One entry per
 *  input point, in order; null where the DEM has no coverage. The server caps a
 *  request at 4096 points. */
export async function sampleElevations(points: LatLng[]): Promise<(number | null)[]> {
  const r = await fetch(`${API_BASE}/v1/elev/samples`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ points: points.map((p) => [p.lng, p.lat]) }),
  });
  if (!r.ok) throw new Error(`elev/samples ${r.status}`);
  const body = (await r.json()) as { elev_m: (number | null)[] };
  return body.elev_m ?? [];
}
