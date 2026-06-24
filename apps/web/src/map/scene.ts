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
  | { type: 'hillshade'; id: string; source: string };

export interface Scene {
  sources: Record<string, SourceDef>;
  layers: Layer[];
}

import { API_BASE } from '../config';

/** The base map a scene is built around. */
export type BaseLayerId = 'norgeskart' | 'topo' | 'osm' | 'satellite';

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

/** All selectable base layers. The Kartverket N50 raster is served by our own
 *  tileserver (`/v1/raster`); the rest are public XYZ sources that send
 *  `Access-Control-Allow-Origin: *`, so the WASM host can `fetch()` + ingest
 *  them cross-origin. 3D terrain (the DEM) is independent of the imagery, so it
 *  stays the Kartverket DEM for every base. */
export const BASE_LAYERS: Record<BaseLayerId, BaseLayerDef> = {
  norgeskart: {
    label: 'Topo (Norge)',
    icon: 'terrain',
    url: `${API_BASE}/v1/raster/n50/{z}/{x}/{y}.png`,
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

/** Build the base scene for a given base layer. The source id stays `basemap`
 *  across layers so a switch is a same-id source-URL swap (the engine
 *  re-ingests); overlays compose on top. */
export function buildBaseScene(base: BaseLayerId): Scene {
  const def = BASE_LAYERS[base];
  return {
    sources: {
      basemap: {
        type: 'raster-xyz',
        tiles: [def.url],
        tile_size: 256,
        min_zoom: 0,
        max_zoom: def.maxZoom,
        attribution: def.attribution,
      },
    },
    layers: [{ type: 'raster', id: 'basemap', source: 'basemap' }],
  };
}
