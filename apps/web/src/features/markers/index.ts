/** `markers` feature slice — place/POI pins + their detail & editor panels.
 *  Panels show via the host mutex (`marker-detail`/`marker-edit`/`marker-new`);
 *  the selection store holds only the payload (selected id / draft point). */
export { MarkerPins } from './MarkerPins';
export { MarkerDetailPanel } from './MarkerDetailPanel';
export { MarkerEditorPanel } from './MarkerEditorPanel';
export { useMarkers, useDeleteMarker } from './useMarkers';
export { useSelection } from './selectionStore';
export { openMarkerDetail, openMarkerEditor, openNewMarker, closeMarker } from './actions';
export { reverseGeocode, type Marker } from './api';
