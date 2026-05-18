import '../models/marker_photo.dart';

/// Persistence interface for marker photo metadata. Mirrors the pattern of
/// [MarkerDataStore] — a SQLite impl backs mobile/desktop, an IndexedDB impl
/// backs web. The actual photo bytes are stored separately by
/// `PhotoStorageService`.
abstract class MarkerPhotoDataStore {
  Future<void> init();
  Future<void> insert(MarkerPhoto photo);
  Future<List<MarkerPhoto>> getByMarker(String markerUuid);
  Future<MarkerPhoto?> getByUuid(String uuid);
  Future<void> delete(String uuid);

  /// Cascade — used when a marker is deleted.
  Future<void> deleteAllForMarker(String markerUuid);
}
