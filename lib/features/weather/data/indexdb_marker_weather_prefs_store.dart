import 'package:idb_shim/idb_browser.dart';

import '../models/marker_weather_prefs.dart';
import 'marker_weather_prefs_store.dart';

class IndexedDBMarkerWeatherPrefsStore implements MarkerWeatherPrefsStore {
  Database? _database;
  final IdbFactory _idbFactory;
  static const String dbName = 'WeatherPrefsDatabaseV1';
  static const String storeName = markerWeatherPrefsTable;
  static const int _dbVersion = 1;

  IndexedDBMarkerWeatherPrefsStore({IdbFactory? idbFactory})
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

  void _onUpgradeNeeded(VersionChangeEvent event) {
    final db = event.database;
    if (!db.objectStoreNames.contains(storeName)) {
      db.createObjectStore(storeName, keyPath: 'marker_uuid');
    }
  }

  @override
  Future<MarkerWeatherPrefs?> get(String markerUuid) async {
    final db = await _db;
    final txn = db.transaction(storeName, 'readonly');
    final store = txn.objectStore(storeName);
    final raw = await store.getObject(markerUuid) as Map<String, dynamic>?;
    await txn.completed;
    if (raw == null) return null;
    final metrics = raw['metrics'];
    if (metrics is! Map<String, dynamic>) {
      return MarkerWeatherPrefs.defaults(markerUuid);
    }
    return MarkerWeatherPrefs.fromJson(markerUuid, metrics);
  }

  @override
  Future<void> upsert(MarkerWeatherPrefs prefs) async {
    final db = await _db;
    final txn = db.transaction(storeName, 'readwrite');
    final store = txn.objectStore(storeName);
    await store.put({
      'marker_uuid': prefs.markerUuid,
      'metrics': prefs.toJson(),
    });
    await txn.completed;
  }

  @override
  Future<void> delete(String markerUuid) async {
    final db = await _db;
    final txn = db.transaction(storeName, 'readwrite');
    final store = txn.objectStore(storeName);
    await store.delete(markerUuid);
    await txn.completed;
  }
}
