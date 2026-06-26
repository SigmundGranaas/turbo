/** `map-engine` — the rendering substrate: the `TurboMap` WASM lifecycle, the
 *  map surface + rAF loop, host-driven tile IO, scene construction, and the
 *  gesture controller. Implements `map-core`'s `MapEngine` contract (published
 *  by `<MapSurface>`); depends on `map-core` + `shared` only — never features
 *  or the host. */
export { MapSurface } from './MapSurface';
export type { CameraInit } from './MapSurface';

export {
  BASE_LAYERS,
  baseSourceId,
  basemapGain,
  buildBaseScene,
} from './scene';
export type { BaseLayerId, BaseLayerDef, Scene, SourceDef, Layer } from './scene';
