import { API_BASE } from '../config';
import { BASE_LAYERS, baseSourceId, type BaseLayerId } from './scene';

/** Tile kinds the engine's streaming plan asks the host to fetch. */
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
      [baseSourceId(base)]: BASE_LAYERS[base].url,
    },
    terrain: {
      // `?halo=1` MUST match the scene's DEM `halo` (scene.ts TERRAIN_HALO) so
      // the engine gets the 258px haloed tiles it meshes crack-free.
      __terrain: `${API_BASE}/v1/dem/rgb/{z}/{x}/{y}.png?halo=1`,
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
