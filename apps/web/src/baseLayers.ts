/** The base-layer id vocabulary — a shared contract between the `map-engine`
 *  (which owns the actual layer catalog + scene/source defs keyed by these ids),
 *  the `uiStore` (which persists the user's selection), and the host's
 *  LayerPicker. Kept here in `shared` so the store can name it without reaching
 *  up into the engine substrate. */
export type BaseLayerId = 'norgeskart' | 'topo' | 'osm' | 'satellite';
