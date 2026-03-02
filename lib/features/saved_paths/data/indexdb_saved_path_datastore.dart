import 'package:idb_shim/idb_browser.dart';
import 'package:latlong2/latlong.dart';
import '../models/saved_path.dart';
import 'saved_path_data_store.dart';

class ShimDBSavedPathDataStore implements SavedPathDataStore {
  Database? _database;
  final IdbFactory _idbFactory;
  static const String dbName = 'SavedPathsDatabaseV1';
  static const String storeName = 'saved_paths';
  static const int _dbVersion = 1;

  ShimDBSavedPathDataStore({IdbFactory? idbFactory})
      : _idbFactory = idbFactory ?? getIdbFactory()!;

  Future<Database> get _db async {
    if (_database != null) return _database!;
    _database = await _idbFactory.open(dbName, version: _dbVersion, onUpgradeNeeded: _onUpgradeNeeded);
    return _database!;
  }

  @override
  Future<void> init() async {
    await _db;
  }

  void _onUpgradeNeeded(VersionChangeEvent event) {
    final db = event.database;
    if (!db.objectStoreNames.contains(storeName)) {
      db.createObjectStore(storeName, keyPath: 'uuid');
    }
  }

  @override
  Future<void> insert(SavedPath path) async {
    final db = await _db;
    final txn = db.transaction(storeName, 'readwrite');
    final store = txn.objectStore(storeName);
    await store.put(path.toLocalMap());
    await txn.completed;
  }

  @override
  Future<SavedPath?> getByUuid(String uuid) async {
    final db = await _db;
    final txn = db.transaction(storeName, 'readonly');
    final store = txn.objectStore(storeName);
    final data = await store.getObject(uuid) as Map<String, dynamic>?;
    await txn.completed;
    return data != null ? SavedPath.fromLocalMap(data) : null;
  }

  @override
  Future<List<SavedPath>> getAll() async {
    final db = await _db;
    final txn = db.transaction(storeName, 'readonly');
    final store = txn.objectStore(storeName);
    final records = await store.getAll();
    await txn.completed;
    return records.map((record) => SavedPath.fromLocalMap(record as Map<String, dynamic>)).toList();
  }

  @override
  Future<void> update(SavedPath path) async {
    await insert(path);
  }

  @override
  Future<void> delete(String uuid) async {
    final db = await _db;
    final txn = db.transaction(storeName, 'readwrite');
    final store = txn.objectStore(storeName);
    await store.delete(uuid);
    await txn.completed;
  }

  @override
  Future<List<SavedPath>> findInBounds(LatLng southwest, LatLng northeast) async {
    final allPaths = await getAll();
    return allPaths.where((path) {
      final b = path.bounds;
      return b.southWest.latitude <= northeast.latitude &&
          b.northEast.latitude >= southwest.latitude &&
          b.southWest.longitude <= northeast.longitude &&
          b.northEast.longitude >= southwest.longitude;
    }).toList();
  }
}
