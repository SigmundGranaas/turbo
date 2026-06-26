/** `map-core` — the passive map kernel: cross-cutting contracts + shared hooks
 *  that feature slices depend on. Depends on `shared`/`ui` only; never on
 *  features, the host, or the `map-engine` implementation. */
export type { MapEngine } from './engine';
export { MapEngineProvider, useMapEngine, useMapEnginePublisher } from './MapEngineContext';
export { useProjectedLayer, viewportDpr } from './useProjectedLayer';
export { UserLocationLayer } from './UserLocation';
export { usePanelHost, type PanelId } from './panelHost';
