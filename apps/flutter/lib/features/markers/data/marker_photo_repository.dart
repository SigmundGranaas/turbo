import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/data/database_provider.dart';

import '../models/marker_photo.dart';
import 'idb_marker_photo_datastore.dart';
import 'marker_photo_data_store.dart';
import 'photo_storage_service.dart';
import 'sqlite_marker_photo_datastore.dart';

/// The file storage helper. Overridable in tests.
final photoStorageServiceProvider = Provider<PhotoStorageService>(
  (ref) => PhotoStorageService(),
);

/// Local-only data store binding. SQLite on mobile/desktop; IndexedDB on web.
/// Overridable in tests so we can drop in an in-memory implementation.
final localMarkerPhotoDataStoreProvider =
    FutureProvider<MarkerPhotoDataStore>((ref) async {
  if (kIsWeb) {
    final store = IdbMarkerPhotoDataStore();
    await store.init();
    return store;
  }
  final db = await ref.watch(databaseProvider.future);
  return SQLiteMarkerPhotoDataStore(db);
});

/// Mutation surface for marker photos. Held by `markerPhotoServiceProvider`.
/// Reads go through `markerPhotosProvider` so UI can subscribe.
class MarkerPhotoService {
  final MarkerPhotoDataStore _store;
  final PhotoStorageService _storage;
  final Ref _ref;

  MarkerPhotoService(this._store, this._storage, this._ref);

  Future<List<MarkerPhoto>> photosFor(String markerUuid) =>
      _store.getByMarker(markerUuid);

  Future<MarkerPhoto> addPhoto({
    required String markerUuid,
    required File source,
  }) async {
    final photo = MarkerPhoto(markerUuid: markerUuid, filePath: '');
    final ext = source.path.contains('.') ? source.path.split('.').last : 'jpg';
    final filename = '${photo.uuid}.$ext';
    final destPath = await _storage.savePhoto(source, filename: filename);
    final stored = MarkerPhoto(
      uuid: photo.uuid,
      markerUuid: markerUuid,
      filePath: destPath,
      createdAt: photo.createdAt,
    );
    await _store.insert(stored);
    _ref.invalidate(markerPhotosProvider(markerUuid));
    return stored;
  }

  Future<void> removePhoto(String photoUuid) async {
    final existing = await _store.getByUuid(photoUuid);
    if (existing == null) return;
    await _store.delete(photoUuid);
    await _storage.deletePhoto(existing.filePath);
    _ref.invalidate(markerPhotosProvider(existing.markerUuid));
  }

  /// Wipe all photos belonging to a deleted marker; used by the marker
  /// deletion cascade.
  Future<void> deleteAllForMarker(String markerUuid) async {
    final all = await _store.getByMarker(markerUuid);
    for (final p in all) {
      await _storage.deletePhoto(p.filePath);
    }
    await _store.deleteAllForMarker(markerUuid);
    _ref.invalidate(markerPhotosProvider(markerUuid));
  }
}

final markerPhotoServiceProvider =
    FutureProvider<MarkerPhotoService>((ref) async {
  final store = await ref.watch(localMarkerPhotoDataStoreProvider.future);
  final storage = ref.watch(photoStorageServiceProvider);
  return MarkerPhotoService(store, storage, ref);
});

/// Async list of photos for a single marker. Family arg is the marker uuid.
final markerPhotosProvider =
    FutureProvider.autoDispose.family<List<MarkerPhoto>, String>(
  (ref, markerUuid) async {
    final svc = await ref.watch(markerPhotoServiceProvider.future);
    return svc.photosFor(markerUuid);
  },
);
