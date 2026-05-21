import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/markers/data/sqlite_marker_photo_datastore.dart';

import '../../helpers/in_memory_db.dart';

/// A stub storage that writes a tiny in-memory map instead of touching disk.
/// Real PhotoStorageService is integration-tested only on a real device.
class _StubStorage implements PhotoStorageService {
  final Map<String, String> saved = {};
  final List<String> deleted = [];

  @override
  Future<String> savePhoto(File source, {required String filename}) async {
    final path = '/fake/$filename';
    saved[path] = source.path;
    return path;
  }

  @override
  Future<void> deletePhoto(String filePath) async {
    deleted.add(filePath);
    saved.remove(filePath);
  }
}

void main() {
  late Database db;
  late MarkerPhotoDataStore store;
  late _StubStorage storage;
  late ProviderContainer container;

  setUp(() async {
    db = await createMarkersDb();
    store = SQLiteMarkerPhotoDataStore(db);
    storage = _StubStorage();
    container = ProviderContainer(overrides: [
      localMarkerPhotoDataStoreProvider.overrideWith((ref) async => store),
      photoStorageServiceProvider.overrideWithValue(storage),
    ]);
  });

  tearDown(() async {
    container.dispose();
    await db.close();
  });

  test('addPhoto persists row + copies file via storage service', () async {
    final svc = await container.read(markerPhotoServiceProvider.future);
    final source = File('/tmp/fake-source.jpg');
    final photo = await svc.addPhoto(markerUuid: 'm-1', source: source);

    expect(photo.markerUuid, 'm-1');
    expect(photo.filePath, isNotEmpty);
    expect(storage.saved.values, contains('/tmp/fake-source.jpg'));

    final all = await store.getByMarker('m-1');
    expect(all.length, 1);
    expect(all.first.uuid, photo.uuid);
  });

  test('removePhoto deletes both row and file', () async {
    final svc = await container.read(markerPhotoServiceProvider.future);
    final photo =
        await svc.addPhoto(markerUuid: 'm-2', source: File('/tmp/a.jpg'));

    await svc.removePhoto(photo.uuid);

    expect(await store.getByUuid(photo.uuid), isNull);
    expect(storage.deleted, contains(photo.filePath));
  });

  test('photosFor returns photos sorted by created_at ascending', () async {
    final svc = await container.read(markerPhotoServiceProvider.future);
    final p1 = MarkerPhoto(
      markerUuid: 'm-3',
      filePath: '/x/1.jpg',
      createdAt: DateTime(2026, 5, 1),
    );
    final p2 = MarkerPhoto(
      markerUuid: 'm-3',
      filePath: '/x/2.jpg',
      createdAt: DateTime(2026, 5, 2),
    );
    await store.insert(p2);
    await store.insert(p1);

    final all = await svc.photosFor('m-3');
    expect(all.map((p) => p.uuid), [p1.uuid, p2.uuid]);
  });

  test('deleteAllForMarker wipes all photos and file artifacts', () async {
    final svc = await container.read(markerPhotoServiceProvider.future);
    await svc.addPhoto(markerUuid: 'm-4', source: File('/tmp/x.jpg'));
    await svc.addPhoto(markerUuid: 'm-4', source: File('/tmp/y.jpg'));

    await svc.deleteAllForMarker('m-4');

    final remaining = await store.getByMarker('m-4');
    expect(remaining, isEmpty);
    expect(storage.deleted.length, 2);
  });
}
