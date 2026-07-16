/** The base-layer id vocabulary — a shared contract between the `map-engine`
 *  (which owns the actual layer catalog + scene/source defs keyed by these ids),
 *  the `uiStore` (which persists the user's selection), and the host's
 *  LayerPicker. Kept here in `shared` so the store can name it without reaching
 *  up into the engine substrate.
 *
 *  Widened to `string`: alongside the built-ins the id may name a user-added
 *  custom XYZ source (`custom-<uuid>`, persisted in the uiStore and registered
 *  with the scene via `setCustomBaseLayers`). `BUILTIN_BASE_LAYERS` keeps the
 *  known vocabulary for exhaustiveness where it matters. */
export type BaseLayerId = string;

export const BUILTIN_BASE_LAYERS = ['norgeskart', 'topo', 'osm', 'satellite'] as const;

/** A user-added XYZ basemap ("add your own map URL"), persisted in the uiStore. */
export interface CustomBaseLayer {
  id: BaseLayerId;
  label: string;
  url: string;
  maxZoom: number;
}

/** True when [url] is a usable XYZ template: http(s) + all of {z}/{x}/{y}.
 *  Mirrors the Android `CustomTileSource.isValidTemplate` rule. */
export function isValidXyzTemplate(url: string): boolean {
  const u = url.trim();
  const scheme = u.startsWith('http://') || u.startsWith('https://');
  return scheme && ['{z}', '{x}', '{y}'].every((p) => u.includes(p));
}
