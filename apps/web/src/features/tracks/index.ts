/** `tracks` feature slice — saved paths/tracks: the "Saved → Paths" list, a
 *  track's detail (elevation, export, share, add-to-collection), and the
 *  name/colour/icon editor, plus GPX/KML/GeoJSON import. Shown in the host's
 *  `saved` panel slot; sub-navigation (list ↔ detail ↔ edit) lives in the
 *  shared `pathsStore`. The track data layer is `./api`. */
export { PathsListPanel } from './PathsListPanel';
export { PathDetailPanel } from './PathDetailPanel';
export { TrackEditorPanel } from './TrackEditorPanel';
export { useTracks, useCreateTrack, useUpdateTrack, useDeleteTrack } from './useTracks';
export { serializeTrack, dashArrayFor, type Track, type TrackInput, type TrackChanges, type ExportFormat } from './api';
export { parseTrack, trackStats } from './trackImport';
