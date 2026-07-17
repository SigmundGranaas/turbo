/** TypeScript mirror of the turbomap Scene IR (`turbomap-scene/src/scene.rs`).
 *  `tag="type"` kebab-case variants; struct fields stay snake_case. The engine
 *  parses exactly this JSON via `apply_scene`. Kept minimal for Phase 1 (base
 *  raster + DEM + a generic line layer for routes/tracks); widen per feature in
 *  Phase 2 to match the Rust enum. */

export type DemEncoding = 'mapbox-rgb' | 'terrarium';

export type SourceDef =
  | {
      type: 'raster-xyz';
      tiles: string[];
      tile_size?: number;
      min_zoom?: number;
      max_zoom?: number;
      attribution?: string;
    }
  | { type: 'vector-xyz'; tiles: string[]; min_zoom?: number; max_zoom?: number }
  | { type: 'geo-json'; data: string }
  | {
      type: 'dem-xyz';
      tiles: string[];
      encoding: DemEncoding;
      min_zoom?: number;
      max_zoom?: number;
      halo?: number;
    };

/** IR color — the serde shape of `turbomap_scene::Color`. */
export interface SceneColor {
  r: number;
  g: number;
  b: number;
  a: number;
}

export type Layer =
  | { type: 'raster'; id: string; source: string }
  | {
      type: 'hillshade';
      id: string;
      source: string;
      exaggeration?: number;
      /** true = the DEM is height-only (displaces the ground; no relief overlay
       *  drawn — the basemap raster lights itself from the sun). */
      height_only?: boolean;
    }
  // Content layers (plan P6.3) — routes/tracks, pins, the location dot are
  // scene-declared over geo-json sources; DOM is for interactive chrome only.
  | {
      type: 'line';
      id: string;
      source: string;
      color: SceneColor;
      width: number;
      /** `[dash, gap]` in screen px; omitted = solid. */
      dash_array?: number[];
    }
  | { type: 'circle'; id: string; source: string; color: SceneColor; radius: number }
  | { type: 'tube'; id: string; source: string; color: SceneColor; radius_px: number };

// The scene-declared environment (plan P5.2) and content (plan P6.3) planes.
// The types + live stores are in map-core (`environment.ts`, `mapContent.ts`)
// so features can publish without importing the engine substrate; the surface
// subscribes there and re-applies the scene built here.
import {
  currentEnvironment,
  currentMapContent,
  type MapContent,
  type MapEnvironment,
} from '../map-core';
export { onEnvironmentChange, onMapContentChange } from '../map-core';

export interface Scene {
  sources: Record<string, SourceDef>;
  layers: Layer[];
  environment?: MapEnvironment;
}

import { API_BASE } from '../config';

/** Shared Kartverket DEM for 3D terrain — independent of the chosen imagery, so
 *  it's in every scene. `?halo=1` makes the server bake a 1px elevation ring so
 *  adjacent terrain tiles mesh crack-free; the scene `halo` MUST match it. Same
 *  values as Android (`TurbomapScene`: mapbox-rgb, halo 1, exaggeration 6). */
const TERRAIN_DEM_URL = `${API_BASE}/v1/dem/rgb/{z}/{x}/{y}.png?halo=1`;
const TERRAIN_HALO = 1;
const TERRAIN_EXAGGERATION = 6.0;

/** The base map a scene is built around (id vocabulary lives in shared). */
export type { BaseLayerId } from '../baseLayers';
import type { BaseLayerId } from '../baseLayers';

export interface BaseLayerDef {
  label: string;
  /** Material Symbols icon for the layer picker. */
  icon: string;
  /** `{z}/{x}/{y}` XYZ template (host fetches + ingests; no `{s}` — the engine
   *  only substitutes z/x/y, so we pin a single subdomain). */
  url: string;
  maxZoom: number;
  attribution: string;
}

/** All selectable base layers — every one is an official public XYZ/WMTS
 *  service that sends `Access-Control-Allow-Origin: *`, so the WASM host can
 *  `fetch()` + ingest them cross-origin. We deliberately do NOT use our own
 *  tileserver's `/v1/raster` here: it only pre-renders a limited area, so it
 *  showed blank outside that box. Kartverket's national cache covers all of
 *  Norway at full zoom. 3D terrain (the DEM) is independent of the imagery, so
 *  it stays the Kartverket DEM for every base.
 *
 *  Note the Kartverket WMTS path order is `{z}/{y}/{x}` (TileMatrix/Row/Col);
 *  `tileUrl` substitutes by name, so the order in the string is honoured. */
export const BASE_LAYERS: Record<BaseLayerId, BaseLayerDef> = {
  norgeskart: {
    label: 'Topo (Norge)',
    icon: 'terrain',
    url: 'https://cache.kartverket.no/v1/wmts/1.0.0/topo/default/webmercator/{z}/{y}/{x}.png',
    maxZoom: 18,
    attribution: '© Kartverket',
  },
  topo: {
    label: 'Topo (world)',
    icon: 'landscape',
    url: 'https://a.tile.opentopomap.org/{z}/{x}/{y}.png',
    maxZoom: 17,
    attribution: '© OpenTopoMap (CC-BY-SA)',
  },
  osm: {
    label: 'Street (OSM)',
    icon: 'map',
    url: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
    maxZoom: 19,
    attribution: '© OpenStreetMap contributors',
  },
  satellite: {
    label: 'Satellite',
    icon: 'satellite_alt',
    url: 'https://mt1.google.com/vt/lyrs=s&x={x}&y={y}&z={z}',
    maxZoom: 20,
    attribution: '© Google',
  },
};

/** The engine source/layer id for a base. Crucially this is *per base* (not a
 *  shared `basemap`): a switch then targets a fresh layer id with no cached
 *  tiles, so the engine re-requests imagery from the new URL. Reusing one id
 *  left the previously-ingested raster on screen — the "layers don't render"
 *  bug — because the engine never re-fetched for an id it already had. */
export function baseSourceId(base: BaseLayerId): string {
  return `basemap-${base}`;
}

// User-added XYZ basemaps, registered by the host (a React effect syncs the
// uiStore's persisted list here — scene building stays store-agnostic).
let customBaseLayers: Record<string, BaseLayerDef> = {};

/** Replace the registered custom base layers (keyed by id). */
export function setCustomBaseLayers(defs: Record<string, BaseLayerDef>): void {
  customBaseLayers = defs;
}

/** Resolve a base-layer id: built-in catalog first, then the user's custom
 *  sources; unknown ids (e.g. a deleted custom layer still persisted as the
 *  selection) fall back to the default topo rather than a blank map. */
export function resolveBaseLayer(base: BaseLayerId): BaseLayerDef {
  return BASE_LAYERS[base] ?? customBaseLayers[base] ?? BASE_LAYERS.norgeskart;
}

/** 3D sun-lit basemap brightness gain. Satellite imagery is intrinsically dark
 *  (forest/rock + baked lighting), so under the terrain sun-lighting it reads
 *  near-black at the brightness that suits bright topo — lift it. Only affects
 *  the 3D lit path (2D is flat/untouched). Tune by eye. */
export function basemapGain(base: BaseLayerId): number {
  return base === 'satellite' ? 1.8 : 1.0;
}

/** Build the base scene for a given base layer. `terrain` (the derived
 *  `demPresent`) adds the DEM heightmap + height-only hillshade so the map shows
 *  relief — needed for 3D AND for the 2D sun-lit top-down case ("3D seen from
 *  the top"). Plain 2D (no 3D, no sun) omits it, staying a flat, bright,
 *  perfectly-registered map. `exaggeration` is the derived vertical
 *  exaggeration (the 3D slider's value, or the default detent when sun lights a
 *  flat 2D map). */
export function buildBaseScene(base: BaseLayerId, terrain = false, exaggeration = TERRAIN_EXAGGERATION): Scene {
  const def = resolveBaseLayer(base);
  const id = baseSourceId(base);
  const sources: Scene['sources'] = {
    [id]: {
      type: 'raster-xyz',
      tiles: [def.url],
      tile_size: 256,
      min_zoom: 0,
      max_zoom: def.maxZoom,
      attribution: def.attribution,
    },
  };
  const layers: Layer[] = [{ type: 'raster', id, source: id }];
  // The scene carries the whole environment (P5.2): the per-base brightness
  // gain folds in here; user-driven fields ride the live environment store.
  const environment: MapEnvironment = { ...currentEnvironment(), 'basemap-gain': basemapGain(base) };
  if (terrain) {
    // Host-driven: bytes come from the `__terrain` template (templates.ts), but
    // the engine needs the source declared to know encoding/halo + request it.
    sources.dem = { type: 'dem-xyz', tiles: [TERRAIN_DEM_URL], encoding: 'mapbox-rgb', halo: TERRAIN_HALO };
    // height_only: displace the ground, no relief overlay — the basemap lights
    // itself from the sun (one lit 3D surface), same as Android.
    layers.push({ type: 'hillshade', id: 'hillshade', source: 'dem', exaggeration, height_only: true });
  }
  appendContent(sources, layers, currentMapContent());
  return { sources, layers, environment };
}

/** Resolve a CSS color (hex or `var(--...)`) to the IR shape. Guarded for
 *  non-DOM environments (vitest): unresolvable values fall back to the theme
 *  primary blue. */
export function cssColorToScene(css: string | undefined, alpha = 255): SceneColor {
  const FALLBACK: SceneColor = { r: 37, g: 99, b: 235, a: alpha };
  let v = (css ?? '').trim();
  if (v.startsWith('var(') && typeof document !== 'undefined') {
    const name = v.slice(4, -1).trim();
    v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }
  const m = /^#?([0-9a-f]{6})$/i.exec(v.startsWith('#') ? v.slice(1) : v);
  if (!m) return FALLBACK;
  const n = parseInt(m[1], 16);
  return { r: (n >> 16) & 0xff, g: (n >> 8) & 0xff, b: n & 0xff, a: alpha };
}

const WHITE: SceneColor = { r: 255, g: 255, b: 255, a: 255 };
const LOCATION_BLUE: SceneColor = { r: 37, g: 99, b: 235, a: 255 };

function lineString(coords: { lat: number; lng: number }[]): string {
  return JSON.stringify({
    type: 'LineString',
    coordinates: coords.map((c) => [c.lng, c.lat]),
  });
}

function multiPoint(coords: { lat: number; lng: number }[]): string {
  return JSON.stringify({
    type: 'MultiPoint',
    coordinates: coords.map((c) => [c.lng, c.lat]),
  });
}

function pinFeatures(pins: { id: string; lat: number; lng: number }[]): string {
  return JSON.stringify({
    type: 'FeatureCollection',
    features: pins.map((p) => ({
      type: 'Feature',
      properties: { id: p.id },
      geometry: { type: 'Point', coordinates: [p.lng, p.lat] },
    })),
  });
}

/** Fold the live content plane (route/track lines, pins, the location fix)
 *  into the scene as geo-json sources + content layers (plan P6.3). Emission
 *  order = paint order within the overlay track: lines under pins under the
 *  location dot. */
function appendContent(sources: Scene['sources'], layers: Layer[], content: MapContent): void {
  // Named lines (route preview, selected track). Halo under stroke replicates
  // the old SVG look; dashed marks an unsolved preview.
  for (const [owner, line] of Object.entries(content.lines)) {
    const src = `content-line-${owner}`;
    sources[src] = { type: 'geo-json', data: lineString(line.coords) };
    layers.push({
      type: 'line',
      id: `${src}-halo`,
      source: src,
      color: { r: 255, g: 255, b: 255, a: 178 },
      width: 9,
    });
    layers.push({
      type: 'line',
      id: src,
      source: src,
      color: cssColorToScene(line.color),
      width: 5,
      ...(line.dash ? { dash_array: line.dash } : line.dashed ? { dash_array: [2, 10] } : {}),
    });
  }

  // Marker pins: one circle pair (white backing ring + kind-coloured dot) per
  // distinct kind colour; the selected pin gets an emphasized pair on top.
  // Each pin is a FEATURE carrying its domain id — the engine's hit test
  // answers a tap with `properties.id`, so hosts never do geometry math.
  const pins = content.pins;
  if (pins.length > 0) {
    const byColor = new Map<string, { id: string; lat: number; lng: number }[]>();
    for (const p of pins) {
      if (p.id === content.selectedPinId) continue;
      const list = byColor.get(p.color) ?? [];
      list.push(p);
      byColor.set(p.color, list);
    }
    let i = 0;
    for (const [color, pts] of byColor) {
      const src = `content-pins-${i++}`;
      sources[src] = { type: 'geo-json', data: pinFeatures(pts) };
      layers.push({ type: 'circle', id: `${src}-ring`, source: src, color: WHITE, radius: 11 });
      layers.push({ type: 'circle', id: src, source: src, color: cssColorToScene(color), radius: 8 });
    }
    const sel = pins.find((p) => p.id === content.selectedPinId);
    if (sel) {
      sources['content-pin-selected'] = { type: 'geo-json', data: pinFeatures([sel]) };
      layers.push({ type: 'circle', id: 'content-pin-selected-ring', source: 'content-pin-selected', color: WHITE, radius: 15 });
      layers.push({ type: 'circle', id: 'content-pin-selected', source: 'content-pin-selected', color: cssColorToScene(sel.color), radius: 11 });
    }
  }

  // The user-location fix: soft accuracy halo + white-ringed dot. The dot colour
  // is a persisted user setting; default blue.
  if (content.userFix) {
    const dot = content.userFixColor ? cssColorToScene(content.userFixColor) : LOCATION_BLUE;
    sources['content-user-location'] = { type: 'geo-json', data: multiPoint([content.userFix]) };
    layers.push({
      type: 'circle',
      id: 'content-user-location-halo',
      source: 'content-user-location',
      color: { ...dot, a: 46 },
      radius: 22,
    });
    layers.push({ type: 'circle', id: 'content-user-location-ring', source: 'content-user-location', color: WHITE, radius: 11 });
    layers.push({ type: 'circle', id: 'content-user-location', source: 'content-user-location', color: dot, radius: 8 });
  }
}
