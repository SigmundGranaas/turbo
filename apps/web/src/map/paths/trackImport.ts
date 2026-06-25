import { type LatLng, haversineMeters } from '../../geo';

/** A track parsed from a GPX / KML / GeoJSON file. Elevations are parallel to
 *  points (null where the file omitted one). */
export interface ParsedTrack {
  name?: string;
  points: LatLng[];
  elevations: (number | null)[];
}

const num = (s: string | null | undefined): number | null => {
  if (s == null || s.trim() === '') return null;
  const n = parseFloat(s);
  return Number.isFinite(n) ? n : null;
};

/** Auto-detect format and parse a track. Returns null if fewer than 2 points.
 *  Ports the Android `TrackImport.parse` tolerance (GPX trkpt/rtept, KML
 *  coordinates, GeoJSON LineString/MultiLineString). */
export function parseTrack(text: string): ParsedTrack | null {
  const t = text.trim();
  let parsed: ParsedTrack | null = null;
  if (t.startsWith('{') || t.startsWith('[')) parsed = parseGeoJson(t);
  else if (/<kml[\s>]/i.test(t)) parsed = parseKml(t);
  else if (/<gpx[\s>]/i.test(t)) parsed = parseGpx(t);
  else parsed = parseGpx(t) ?? parseKml(t) ?? parseGeoJson(t); // last-ditch sniff
  if (!parsed || parsed.points.length < 2) return null;
  return parsed;
}

function parseGpx(text: string): ParsedTrack | null {
  let doc: Document;
  try {
    doc = new DOMParser().parseFromString(text, 'application/xml');
  } catch {
    return null;
  }
  if (doc.querySelector('parsererror')) return null;
  // Prefer track points; fall back to route points.
  let nodes = Array.from(doc.getElementsByTagName('trkpt'));
  if (nodes.length === 0) nodes = Array.from(doc.getElementsByTagName('rtept'));
  if (nodes.length === 0) return null;
  const points: LatLng[] = [];
  const elevations: (number | null)[] = [];
  for (const n of nodes) {
    const lat = num(n.getAttribute('lat'));
    const lng = num(n.getAttribute('lon'));
    if (lat == null || lng == null) continue;
    points.push({ lat, lng });
    elevations.push(num(n.getElementsByTagName('ele')[0]?.textContent ?? null));
  }
  const name = doc.getElementsByTagName('name')[0]?.textContent?.trim() || undefined;
  return { name, points, elevations };
}

function parseKml(text: string): ParsedTrack | null {
  let doc: Document;
  try {
    doc = new DOMParser().parseFromString(text, 'application/xml');
  } catch {
    return null;
  }
  if (doc.querySelector('parsererror')) return null;
  const blocks = Array.from(doc.getElementsByTagName('coordinates'));
  if (blocks.length === 0) return null;
  const points: LatLng[] = [];
  const elevations: (number | null)[] = [];
  for (const block of blocks) {
    const raw = (block.textContent ?? '').trim();
    for (const tuple of raw.split(/\s+/)) {
      const parts = tuple.split(',');
      if (parts.length < 2) continue;
      const lng = num(parts[0]);
      const lat = num(parts[1]);
      if (lat == null || lng == null) continue;
      points.push({ lat, lng });
      elevations.push(num(parts[2] ?? null));
    }
  }
  const name = doc.getElementsByTagName('name')[0]?.textContent?.trim() || undefined;
  return { name, points, elevations };
}

function parseGeoJson(text: string): ParsedTrack | null {
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch {
    return null;
  }
  const points: LatLng[] = [];
  const elevations: (number | null)[] = [];
  let name: string | undefined;

  const pushLine = (coords: unknown) => {
    if (!Array.isArray(coords)) return;
    for (const c of coords) {
      if (!Array.isArray(c) || c.length < 2) continue;
      const lng = num(String(c[0]));
      const lat = num(String(c[1]));
      if (lat == null || lng == null) continue;
      points.push({ lat, lng });
      elevations.push(c.length > 2 ? num(String(c[2])) : null);
    }
  };
  const handleGeometry = (g: { type?: string; coordinates?: unknown } | null | undefined) => {
    if (!g) return;
    if (g.type === 'LineString') pushLine(g.coordinates);
    else if (g.type === 'MultiLineString' && Array.isArray(g.coordinates)) g.coordinates.forEach(pushLine);
  };

  const root = json as { type?: string; geometry?: { type?: string; coordinates?: unknown }; properties?: { name?: string }; features?: unknown[]; coordinates?: unknown };
  if (root.type === 'FeatureCollection' && Array.isArray(root.features)) {
    for (const f of root.features) {
      const feat = f as { geometry?: { type?: string; coordinates?: unknown }; properties?: { name?: string } };
      handleGeometry(feat.geometry);
      if (!name) name = feat.properties?.name;
    }
  } else if (root.type === 'Feature') {
    handleGeometry(root.geometry);
    name = root.properties?.name;
  } else if (root.type === 'LineString' || root.type === 'MultiLineString') {
    handleGeometry({ type: root.type, coordinates: root.coordinates });
  }
  return { name, points, elevations };
}

/** Distance (metres) + ascent/descent from elevation deltas. Mirrors Android
 *  `GeoMetrics`. Elevation totals are undefined when no elevation data exists. */
export function trackStats(points: LatLng[], elevations: (number | null)[]): {
  distanceM: number;
  ascentM?: number;
  descentM?: number;
} {
  let distanceM = 0;
  for (let i = 1; i < points.length; i++) distanceM += haversineMeters(points[i - 1], points[i]);

  const hasEle = elevations.some((e) => e != null);
  if (!hasEle) return { distanceM };
  let ascentM = 0;
  let descentM = 0;
  let prev: number | null = null;
  for (const e of elevations) {
    if (e == null) continue;
    if (prev != null) {
      const d = e - prev;
      if (d > 0) ascentM += d;
      else descentM -= d;
    }
    prev = e;
  }
  return { distanceM, ascentM, descentM };
}
