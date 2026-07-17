/** `map-core` — the passive map kernel: cross-cutting contracts + shared hooks
 *  that feature slices depend on. Depends on `shared`/`ui` only; never on
 *  features, the host, or the `map-engine` implementation. */
export type { MapEngine } from './engine';
export { MapEngineProvider, useMapEngine, useMapEnginePublisher } from './MapEngineContext';
export { useProjectedLayer, viewportDpr } from './useProjectedLayer';
export { UserLocationLayer } from './UserLocation';
export { RouteOverlay } from './RouteOverlay';
export { MapPointMarkers } from './MapPointMarkers';
export { useMapPoints } from './mapPoints';
export { usePanelHost, type PanelId } from './panelHost';
export { useOnline } from './connectivity';
export {
  reduceMapPointCard,
  measureAvailability,
  HIDDEN as MAP_POINT_CARD_HIDDEN,
  shownCard,
  type MapPointCard,
  type MapPointCardEvent,
  type MeasureAvailability,
} from './mapPointCard';
export {
  currentEnvironment,
  onEnvironmentChange,
  setMapEnvironment,
  type MapEnvironment,
} from './environment';
export {
  deriveMapEnvironment,
  DEFAULT_3D_DETENT,
  MAX_3D_EXAGGERATION,
  type DerivedEnv,
} from './mapEnvironment';
export {
  currentMapContent,
  onMapContentChange,
  setMapContent,
  setMapLine,
  type MapContent,
  type MapLine,
  type MapPin,
  type LatLngPoint,
} from './mapContent';
