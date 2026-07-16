/** The scene-declared map CONTENT plane (plan P6.3) — the live store for
 *  everything the map *shows* beyond the basemap: route/track lines, marker
 *  pins, the user-location fix. Features publish data here; the surface
 *  (map-engine) subscribes and re-applies the one Scene document, exactly
 *  like the environment store (`environment.ts`). DOM overlays are for
 *  interactive chrome only (popups, drag handles) — never for map content.
 *
 *  Same tier logic as the environment: this lives in map-core so features can
 *  publish without importing the engine substrate.
 */

export interface LatLngPoint {
  lat: number;
  lng: number;
}

/** A polyline the map draws (planned route, selected track). Keyed by an
 *  owner id so independent features (route planner, track detail) can each
 *  own one without clobbering the other. */
export interface MapLine {
  coords: LatLngPoint[];
  /** CSS color (hex or `var(--...)`); resolved to an IR color at scene build. */
  color?: string;
  /** Dashed preview styling (e.g. while the route solver is still running). */
  dashed?: boolean;
  /** Explicit dash pattern (px) — a track's user-chosen line style. Wins over
   *  [dashed]'s fixed preview pattern when both are set. */
  dash?: number[];
}

/** One marker pin. `color` is the kind tint (CSS hex). */
export interface MapPin {
  id: string;
  lat: number;
  lng: number;
  color: string;
}

export interface MapContent {
  lines: Record<string, MapLine>;
  pins: MapPin[];
  /** The selected pin renders emphasized (bigger, ringed). */
  selectedPinId?: string;
  /** Latest geolocation fix; null hides the location dot. */
  userFix: LatLngPoint | null;
  /** My-position dot colour (CSS hex); null = the default blue. A user setting. */
  userFixColor: string | null;
}

let content: MapContent = { lines: {}, pins: [], selectedPinId: undefined, userFix: null, userFixColor: null };
let listener: (() => void) | undefined;

export function currentMapContent(): MapContent {
  return content;
}

/** Merge a partial update and notify the surface (one re-apply per call —
 *  callers batch their own changes into a single patch). */
export function setMapContent(patch: Partial<MapContent>): void {
  content = { ...content, ...patch };
  listener?.();
}

/** Publish or clear one named line. */
export function setMapLine(owner: string, line: MapLine | null): void {
  const lines = { ...content.lines };
  if (line && line.coords.length >= 2) lines[owner] = line;
  else delete lines[owner];
  setMapContent({ lines });
}

/** The surface's subscription (single listener, like the environment store). */
export function onMapContentChange(cb: (() => void) | undefined): void {
  listener = cb;
}
