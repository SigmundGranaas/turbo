import { API_BASE } from '../config';
import { BASE_LAYERS, type BaseLayerId } from './scene';

/** Tile kinds the engine asks for via `pending_tiles()`. */
export type TileKind = 'raster' | 'terrain' | 'vector' | 'hillshade';

/** Host-side `kind → layer-id → {z}/{x}/{y} URL template` map. The engine only
 *  reports *which* tiles it wants; the host owns the URLs (and, later, auth /
 *  caching / offline). Mirrors the source `tiles` templates in the scene. */
export type Templates = Record<TileKind, Record<string, string>>;

/** Templates for a given base layer: the `basemap` raster URL tracks the
 *  selected layer; `__terrain` is the shared Kartverket DEM (elevation is
 *  independent of imagery), the layer id the engine uses for terrain (see
 *  `surface.rs` TERRAIN_KEY). */
export function templatesFor(base: BaseLayerId): Templates {
  return {
    raster: {
      basemap: BASE_LAYERS[base].url,
    },
    terrain: {
      __terrain: `${API_BASE}/v1/dem/rgb/{z}/{x}/{y}.png`,
    },
    vector: {},
    hillshade: {},
  };
}

/** Default (Kartverket N50) templates. */
export const defaultTemplates: Templates = templatesFor('norgeskart');

export function tileUrl(template: string, z: number, x: number, y: number): string {
  return template
    .replace('{z}', String(z))
    .replace('{x}', String(x))
    .replace('{y}', String(y));
}
