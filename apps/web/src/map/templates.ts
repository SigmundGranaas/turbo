import { API_BASE } from '../config';

/** Tile kinds the engine asks for via `pending_tiles()`. */
export type TileKind = 'raster' | 'terrain' | 'vector' | 'hillshade';

/** Host-side `kind → layer-id → {z}/{x}/{y} URL template` map. The engine only
 *  reports *which* tiles it wants; the host owns the URLs (and, later, auth /
 *  caching / offline). Mirrors the source `tiles` templates in the scene. */
export type Templates = Record<TileKind, Record<string, string>>;

/** Default templates against the live tileserver. `__terrain` is the shared
 *  DEM layer id the engine uses for terrain (see `surface.rs` TERRAIN_KEY). */
export const defaultTemplates: Templates = {
  raster: {
    basemap: `${API_BASE}/v1/raster/n50/{z}/{x}/{y}.png`,
  },
  terrain: {
    __terrain: `${API_BASE}/v1/dem/rgb/{z}/{x}/{y}.png`,
  },
  vector: {},
  hillshade: {},
};

export function tileUrl(template: string, z: number, x: number, y: number): string {
  return template
    .replace('{z}', String(z))
    .replace('{x}', String(x))
    .replace('{y}', String(y));
}
