import 'package:idb_shim/idb_browser.dart';

import '../models/marker_photo.dart';
import 'marker_photo_data_store.dart';

class IdbMarkerPhotoDataStore implements MarkerPhotoDataStore {
  Database? _database;
  final IdbFactory _idbFactory;
  static const String dbName = 'MarkerPhotosDatabaseV1';
  static const String storeName = 'marker_photos';
  static const String markerIndex = 'by_marker_uuid';
  static const int _dbVersion = 1;

  IdbMarkerPhotoDataStore({IdbFactory? idbFactory})
      : _idbFactory = idbFactory ?? getIdbFactory()!;

  Future<Database> get _db async {
    if (_database != null) return _database!;
    _database = await _idbFactory.open(
      dbName,
      version: _dbVersion,
      onUpgradeNeeded: _onUpgradeNeeded,
    );
    return _database!;
  }

  @override
  Future<void> init() async {
    await _db;
  }

  void _onUpgradeNeeded(VersionChangeEvent event) {
    final db = event.database;
    if (!db.objectStoreNames.contains(storeName)) {
      final store = db.createObjectStore(storeName, keyPath: 'uuid');
      store.createIndex(markerIndex, 'marker_uuid', unique: false);
    }
  }

  @override
  Future<void> insert(MarkerPhoto photo) async {
    final db = await _db;
    final txn = db.transaction(storeName, 'readwrite');
    await txn.objectStore(storeName).put(photo.toLocalMap());
    await txn.completed;
  }

  @override
  Future<List<MarkerPhoto>> getByMarker(String markerUuid) async {
    final db = await _db;
    final txn = db.transaction(storeName, 'readonly');
    final store = txn.objectStore(storeName);
    final index = store.index(markerIndex);
    final records = await index.getAll(markerUuid) as List<dynamic>;
    await txn.completed;
    final list = records
        .map((r) => MarkerPhoto.fromLocalMap(r as Map<String, dynamic>))
        .toList();
    list.sort((a, b) => a.createdAt.compareTo(b.createdAt));
    return list;
  }

  @override
  Future<MarkerPhoto?> getByUuid(String uuid) async {
    final db = await _db;
    final txn = db.transaction(storeName, 'readonly');
    final raw = await txn.objectStore(storeName).getObject(uuid)
        as Map<String, dynamic>?;
    await txn.completed;
    return raw == null ? null : MarkerPhoto.fromLocalMap(raw);
  }

  @override
  Future<void> delete(String uuid) async {
    final db = await _db;
    final txn = db.transaction(storeName, 'readwrite');
    await txn.objectStore(storeName).delete(uuid);
    await txn.completed;
  }

  @override
  Future<void> deleteAllForMarker(String markerUuid) async {
    final photos = await getByMarker(markerUuid);
    for (final p in photos) {
      await delete(p.uuid);
    }
  }
}
