import { API_BASE } from '../config';
import { utm33ToLatLng, type LatLng } from '../geo';

export interface PlaceHit {
  name: string;
  kind: string;
  lat: number;
  lng: number;
  /** Secondary line (postal town, county); empty for anchor-index hits. */
  sub?: string;
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

// ── Geonorge backends (addresses + municipalities) ────────────────────────
// Direct calls to the public Kartverket APIs — the same backends the old
// Flutter composite search used (CORS-friendly). The long-term home is a
// Matrikkel ingest into the tileserver index; these can then be swapped out
// without touching the UI.

/** Street-address search (Kartverket Adresser). `kind: 'address'`; the
 *  postal town rides in `sub`. */
export async function searchAddresses(q: string, limit = 5): Promise<PlaceHit[]> {
  const query = q.trim();
  if (query.length < 3) return [];
  const url = `https://ws.geonorge.no/adresser/v1/sok?sok=${encodeURIComponent(query)}&treffPerSide=${limit}`;
  try {
    const res = await fetch(url);
    if (!res.ok) return [];
    const data = (await res.json()) as {
      adresser?: { adressetekst?: string; postnummer?: string; poststed?: string; representasjonspunkt?: { lat?: number; lon?: number } }[];
    };
    return (data.adresser ?? []).flatMap((a) => {
      if (!a.adressetekst || a.representasjonspunkt?.lat == null || a.representasjonspunkt?.lon == null) return [];
      return [{
        name: a.adressetekst,
        kind: 'address',
        sub: [a.postnummer, a.poststed].filter(Boolean).join(' '),
        lat: a.representasjonspunkt.lat,
        lng: a.representasjonspunkt.lon,
      }];
    });
  } catch {
    return [];
  }
}

/** Municipality search (Kartverket Kommuneinfo). Two-step: `/sok` matches by
 *  name, the detail call supplies the centre point + county; capped to the top
 *  3 hits so a broad query stays cheap. `kind: 'kommune'`. */
export async function searchKommuner(q: string): Promise<PlaceHit[]> {
  const query = q.trim();
  if (query.length < 2) return [];
  try {
    const res = await fetch(`https://ws.geonorge.no/kommuneinfo/v1/sok?knavn=${encodeURIComponent(query)}`);
    if (!res.ok) return [];
    const data = (await res.json()) as { kommuner?: { kommunenummer?: string }[] };
    const hits = await Promise.all(
      (data.kommuner ?? []).slice(0, 3).map(async (k) => {
        if (!k.kommunenummer) return null;
        try {
          const d = await fetch(`https://ws.geonorge.no/kommuneinfo/v1/kommuner/${k.kommunenummer}?utkoordsys=4258`);
          if (!d.ok) return null;
          const detail = (await d.json()) as {
            kommunenavnNorsk?: string;
            kommunenavn?: string;
            fylkesnavn?: string;
            punktIOmrade?: { coordinates?: number[] };
          };
          const name = detail.kommunenavnNorsk ?? detail.kommunenavn;
          const [lng, lat] = detail.punktIOmrade?.coordinates ?? [];
          if (!name || lat == null || lng == null) return null;
          return { name, kind: 'kommune', sub: detail.fylkesnavn ?? '', lat, lng } satisfies PlaceHit;
        } catch {
          return null;
        }
      }),
    );
    return hits.filter((h): h is PlaceHit => h != null);
  } catch {
    return [];
  }
}
