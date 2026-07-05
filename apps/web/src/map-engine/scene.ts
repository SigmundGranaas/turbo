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
    };

// The scene-declared environment (plan P5.2: ONE content plane). The type +
// live store are in map-core (`environment.ts`) so features can patch it
// without importing the engine substrate; the surface subscribes there and
// re-applies the scene built here.
import { currentEnvironment, type MapEnvironment } from '../map-core';
export { onEnvironmentChange } from '../map-core';

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

/** 3D sun-lit basemap brightness gain. Satellite imagery is intrinsically dark
 *  (forest/rock + baked lighting), so under the terrain sun-lighting it reads
 *  near-black at the brightness that suits bright topo — lift it. Only affects
 *  the 3D lit path (2D is flat/untouched). Tune by eye. */
export function basemapGain(base: BaseLayerId): number {
  return base === 'satellite' ? 1.8 : 1.0;
}

/** Build the base scene for a given base layer. `terrain` adds the DEM
 *  heightmap + height-only hillshade so 3D shows relief — gated to 3D ONLY:
 *  in 2D the hillshade would sun-shade the flat basemap (wrecking satellite —
 *  it goes near-black) and the engine would unproject against displaced
 *  terrain, so markers/overlays land off the true ground point. 2D must be a
 *  flat, bright, perfectly-registered map; 3D opts terrain in. */
export function buildBaseScene(base: BaseLayerId, terrain = false): Scene {
  const def = BASE_LAYERS[base];
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
    layers.push({ type: 'hillshade', id: 'hillshade', source: 'dem', exaggeration: TERRAIN_EXAGGERATION, height_only: true });
  }
  return { sources, layers, environment };
}
