import 'package:idb_shim/idb_browser.dart';
import 'package:latlong2/latlong.dart';
import '../../model/marker.dart';
import '../marker_data_store.dart';

class ShimDBMarkerDataStore implements MarkerDataStore {
  late Database _db;
  final IdbFactory _idbFactory;
  static const String _dbName = 'MarkersDatabaseV2';
  static const String _storeName = 'markers';
  static const int _dbVersion = 1;

  ShimDBMarkerDataStore({IdbFactory? idbFactory})
      : _idbFactory = idbFactory ?? getIdbFactory()!;

  @override
  Future<void> init() async {
    _db = await _idbFactory.open(_dbName, version: _dbVersion, onUpgradeNeeded: _onUpgradeNeeded);
  }

  void _onUpgradeNeeded(VersionChangeEvent event) {
    final db = event.database;
    if (!db.objectStoreNames.contains(_storeName)) {
      final store = db.createObjectStore(_storeName, keyPath: 'uuid');
      store.createIndex('coords', ['latitude', 'longitude'], unique: false);
      store.createIndex('synced', 'synced', unique: false);
    }
  }

  @override
  Future<void> insert(Marker marker) async {
    final txn = _db.transaction(_storeName, 'readwrite');
    final store = txn.objectStore(_storeName);
    await store.put(marker.toLocalMap());
    await txn.completed;
  }

  @override
  Future<Marker?> getByUuid(String uuid) async {
    final txn = _db.transaction(_storeName, 'readonly');
    final store = txn.objectStore(_storeName);
    final data = await store.getObject(uuid) as Map<String, dynamic>?;
    await txn.completed;
    return data != null ? Marker.fromLocalMap(data) : null;
  }

  @override
  Future<List<Marker>> getAll() async {
    final txn = _db.transaction(_storeName, 'readonly');
    final store = txn.objectStore(_storeName);
    final records = await store.getAll();
    await txn.completed;
    return records.map((record) => Marker.fromLocalMap(record as Map<String, dynamic>)).toList();
  }

  @override
  Future<List<Marker>> getUnsynced() async {
    final txn = _db.transaction(_storeName, 'readonly');
    final store = txn.objectStore(_storeName);
    final index = store.index('synced');
    final records = await index.getAll(0); // 0 for false
    await txn.completed;
    return records.map((record) => Marker.fromLocalMap(record as Map<String, dynamic>)).toList();
  }

  @override
  Future<void> update(Marker marker) async {
    await insert(marker); // put handles insert or update
  }

  @override
  Future<void> delete(String uuid) async {
    final txn = _db.transaction(_storeName, 'readwrite');
    final store = txn.objectStore(_storeName);
    await store.delete(uuid);
    await txn.completed;
  }

  @override
  Future<void>deleteAll(List<String> uuids) async {
    if (uuids.isEmpty) return;
    final txn = _db.transaction(_storeName, 'readwrite');
    final store = txn.objectStore(_storeName);
    for (final uuid in uuids) {
      await store.delete(uuid);
    }
    await txn.completed;
  }

  @override
  Future<void> clearAll() async {
    final txn = _db.transaction(_storeName, 'readwrite');
    final store = txn.objectStore(_storeName);
    await store.clear();
    await txn.completed;
  }

  @override
  Future<List<Marker>> findInBounds(LatLng southwest, LatLng northeast) async {
    final allMarkers = await getAll();
    return allMarkers.where((marker) {
      final lat = marker.position.latitude;
      final lng = marker.position.longitude;
      return lat >= southwest.latitude &&
          lat <= northeast.latitude &&
          lng >= southwest.longitude &&
          lng <= northeast.longitude;
    }).toList();
  }
}