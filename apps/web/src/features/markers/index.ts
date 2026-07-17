/** `markers` feature slice — place/POI pins + their detail & editor panels.
 *  Panels show via the host mutex (`marker-detail`/`marker-edit`/`marker-new`);
 *  the selection store holds only the payload (selected id / draft point). */
export { MarkerPins } from './MarkerPins';
export { WeatherPinChips } from './WeatherPinChips';
export { MarkerDetailPanel } from './MarkerDetailPanel';
export { MarkerEditorPanel } from './MarkerEditorPanel';
export { useMarkers, useDeleteMarker, useCreateMarker } from './useMarkers';
export { useWeatherPinForecasts } from './useWeatherPins';
export { useSelection } from './selectionStore';
export { openMarkerDetail, openMarkerEditor, openNewMarker, closeMarker } from './actions';
export { reverseGeocode, type Marker } from './api';
export {
  weatherPinFetchDecision,
  weatherPinUiState,
  encodeWireIcon,
  decodeWireIcon,
  WEATHER_PIN_STALE_MS,
  type MarkerKind,
  type WeatherSnapshot,
} from './weatherPin';
