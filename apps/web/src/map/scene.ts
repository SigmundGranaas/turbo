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

/** The base map a scene is built around. Each maps to a tileserver endpoint
 *  (see `templates.ts`). Phase 1 ships the Kartverket N50 topo raster. */
export type BaseLayerId = 'norgeskart';

/** Build the base scene for a given base layer. Overlays (markers, routes,
 *  tracks, vector layers) get composed on top in Phase 2. */
export function buildBaseScene(base: BaseLayerId, tileUrl: string): Scene {
  switch (base) {
    case 'norgeskart':
      return {
        sources: {
          basemap: {
            type: 'raster-xyz',
            tiles: [tileUrl],
            tile_size: 256,
            min_zoom: 0,
            max_zoom: 18,
            attribution: '© Kartverket',
          },
        },
        layers: [{ type: 'raster', id: 'basemap', source: 'basemap' }],
      };
  }
}
