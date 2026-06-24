import { API_BASE } from '../config';
import { utm33ToLatLng, type LatLng } from '../geo';

export interface PlaceHit {
  name: string;
  kind: string;
  lat: number;
  lng: number;
}

interface AnchorRow {
  name: string;
  kind: string;
  x: number; // UTM33N easting
  y: number; // UTM33N northing
}

/** Forward place search against the tileserver's name index (the populated
 *  `anchors` table behind the basemap labels — public, no auth). Coordinates
 *  come back in EPSG:25833, converted to lat/lng here. Proximity-biased when a
 *  map centre is given (currently name-only; centre reserved for later). */
export async function searchPlaces(q: string, _near?: LatLng, limit = 8): Promise<PlaceHit[]> {
  const query = q.trim();
  if (query.length < 2) return [];
  const url = `${API_BASE}/v1/search/name?q=${encodeURIComponent(query)}&limit=${limit}`;
  try {
    const res = await fetch(url);
    if (!res.ok) return [];
    const data = (await res.json()) as { anchors?: AnchorRow[] };
    return (data.anchors ?? []).map((a) => {
      const ll = utm33ToLatLng(a.x, a.y);
      return { name: a.name, kind: a.kind, lat: ll.lat, lng: ll.lng };
    });
  } catch {
    return [];
  }
}
