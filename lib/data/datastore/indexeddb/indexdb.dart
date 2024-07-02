import 'package:idb_shim/idb_browser.dart';
import 'package:idb_shim/idb_shim.dart';
import '../../model/marker.dart';
import '../marker_data_store.dart';

class ShimDBMarkerDataStore implements MarkerDataStore {
  late Database _db;
  late final IdbFactory? _idbFactory;
  static const String dbName = 'MarkersDatabase';
  static const int dbVersion = 1;
  static const String storeName = 'markers';

  ShimDBMarkerDataStore({IdbFactory? idbFactory})
      : _idbFactory = idbFactory ?? getIdbFactory()!;

  @override
  Future<void> init() async {
    if(_idbFactory == null){
      return Future.error("Unable to init factory for IDB_SHIM");
    }

    _db = await _idbFactory.open(dbName, version: dbVersion, onUpgradeNeeded: _onUpgradeNeeded);
  }

  void _onUpgradeNeeded(VersionChangeEvent event) {
    final Database db = event.database;
    if (!db.objectStoreNames.contains(storeName)) {
      db.createObjectStore(storeName, keyPath: 'uuid');
    }
  }

  @override
  Future<void> insert(Marker marker) async {
    final Transaction txn = _db.transaction(storeName, 'readwrite');
    final ObjectStore store = txn.objectStore(storeName);
    await store.put(marker.toMap());
    await txn.completed;
  }

  @override
  Future<Marker?> getByUuid(String uuid) async {
    final Transaction txn = _db.transaction(storeName, 'readonly');
    final ObjectStore store = txn.objectStore(storeName);
    final Map<String, dynamic>? data = (await store.getObject(uuid)) as Map<String, dynamic>?;
    await txn.completed;
    return data != null ? Marker.fromMap(data) : null;
  }

  @override
  Future<List<Marker>> getAll() async {
    final Transaction txn = _db.transaction(storeName, 'readonly');
    final ObjectStore store = txn.objectStore(storeName);

    List<Object?> rawData = await store.getAll();
    await txn.completed;

    return rawData.map((object) {
      if (object is! Map<String, dynamic>) {
        throw FormatException('Invalid data in store: $object');
      }
      return Marker.fromMap(object);
    }).toList();
  }

  @override
  Future<void> update(Marker marker) async {
    final Transaction txn = _db.transaction(storeName, 'readwrite');
    final ObjectStore store = txn.objectStore(storeName);
    await store.put(marker.toMap());
    await txn.completed;
  }

  @override
  Future<void> delete(String uuid) async {
    final Transaction txn = _db.transaction(storeName, 'readwrite');
    final ObjectStore store = txn.objectStore(storeName);
    await store.delete(uuid);
    await txn.completed;
  }
}