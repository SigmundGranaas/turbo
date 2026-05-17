import 'package:idb_shim/idb_browser.dart';

import '../models/collection.dart';
import '../models/collection_item_ref.dart';
import 'collection_data_store.dart';

class ShimDBCollectionDataStore implements CollectionDataStore {
  Database? _database;
  final IdbFactory _idbFactory;
  static const String dbName = 'CollectionsDatabaseV1';
  static const String collectionsStore = 'collections';
  static const String itemsStore = 'collection_items';
  static const int _dbVersion = 1;

  ShimDBCollectionDataStore({IdbFactory? idbFactory})
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
    if (!db.objectStoreNames.contains(collectionsStore)) {
      db.createObjectStore(collectionsStore, keyPath: 'uuid');
    }
    if (!db.objectStoreNames.contains(itemsStore)) {
      final store = db.createObjectStore(itemsStore, keyPath: 'id');
      store.createIndex('by_collection', 'collection_uuid', unique: false);
      store.createIndex(
        'by_item',
        ['item_type', 'item_uuid'],
        unique: false,
      );
    }
  }

  String _itemKey(String collectionUuid, CollectionItemRef ref) =>
      '$collectionUuid|${ref.type}|${ref.uuid}';

  @override
  Future<List<Collection>> getAll() async {
    final db = await _db;
    final txn = db.transaction(collectionsStore, 'readonly');
    final store = txn.objectStore(collectionsStore);
    final records = await store.getAll();
    await txn.completed;
    final list = records
        .map((r) => Collection.fromLocalMap(r as Map<String, dynamic>))
        .toList();
    list.sort((a, b) {
      final cmp = a.sortOrder.compareTo(b.sortOrder);
      if (cmp != 0) return cmp;
      return a.createdAt.compareTo(b.createdAt);
    });
    return list;
  }

  @override
  Future<Collection?> getByUuid(String uuid) async {
    final db = await _db;
    final txn = db.transaction(collectionsStore, 'readonly');
    final store = txn.objectStore(collectionsStore);
    final data = await store.getObject(uuid) as Map<String, dynamic>?;
    await txn.completed;
    return data != null ? Collection.fromLocalMap(data) : null;
  }

  @override
  Future<void> insert(Collection collection) async {
    final db = await _db;
    final txn = db.transaction(collectionsStore, 'readwrite');
    final store = txn.objectStore(collectionsStore);
    await store.put(collection.toLocalMap());
    await txn.completed;
  }

  @override
  Future<void> update(Collection collection) async {
    await insert(collection);
  }

  @override
  Future<void> delete(String uuid) async {
    final db = await _db;
    final txn = db.transaction(
      [collectionsStore, itemsStore],
      'readwrite',
    );
    final colStore = txn.objectStore(collectionsStore);
    final itemStore = txn.objectStore(itemsStore);
    await colStore.delete(uuid);
    final index = itemStore.index('by_collection');
    final keys = await index.getAllKeys(uuid);
    for (final k in keys) {
      await itemStore.delete(k);
    }
    await txn.completed;
  }

  @override
  Future<List<CollectionItemRef>> getItems(String collectionUuid) async {
    final db = await _db;
    final txn = db.transaction(itemsStore, 'readonly');
    final store = txn.objectStore(itemsStore);
    final index = store.index('by_collection');
    final records = await index.getAll(collectionUuid);
    await txn.completed;
    final maps = records.cast<Map<String, dynamic>>().toList();
    maps.sort((a, b) => (a['added_at'] as String).compareTo(b['added_at'] as String));
    return maps
        .map((m) => CollectionItemRef(
              type: m['item_type'] as String,
              uuid: m['item_uuid'] as String,
            ))
        .toList();
  }

  @override
  Future<List<String>> getCollectionUuidsFor(CollectionItemRef ref) async {
    final db = await _db;
    final txn = db.transaction(itemsStore, 'readonly');
    final store = txn.objectStore(itemsStore);
    final index = store.index('by_item');
    final records = await index.getAll([ref.type, ref.uuid]);
    await txn.completed;
    return records
        .cast<Map<String, dynamic>>()
        .map((m) => m['collection_uuid'] as String)
        .toList();
  }

  @override
  Future<Map<CollectionItemRef, List<String>>> getMembershipIndex() async {
    final db = await _db;
    final txn = db.transaction(itemsStore, 'readonly');
    final store = txn.objectStore(itemsStore);
    final records = await store.getAll();
    await txn.completed;
    final out = <CollectionItemRef, List<String>>{};
    for (final r in records) {
      final m = r as Map<String, dynamic>;
      final ref = CollectionItemRef(
        type: m['item_type'] as String,
        uuid: m['item_uuid'] as String,
      );
      (out[ref] ??= <String>[]).add(m['collection_uuid'] as String);
    }
    return out;
  }

  @override
  Future<void> addItem(String collectionUuid, CollectionItemRef ref) async {
    final db = await _db;
    final txn = db.transaction(itemsStore, 'readwrite');
    final store = txn.objectStore(itemsStore);
    await store.put({
      'id': _itemKey(collectionUuid, ref),
      'collection_uuid': collectionUuid,
      'item_type': ref.type,
      'item_uuid': ref.uuid,
      'added_at': DateTime.now().toIso8601String(),
    });
    await txn.completed;
  }

  @override
  Future<void> removeItem(String collectionUuid, CollectionItemRef ref) async {
    final db = await _db;
    final txn = db.transaction(itemsStore, 'readwrite');
    final store = txn.objectStore(itemsStore);
    await store.delete(_itemKey(collectionUuid, ref));
    await txn.completed;
  }

  @override
  Future<void> removeItemFromAll(CollectionItemRef ref) async {
    final db = await _db;
    final txn = db.transaction(itemsStore, 'readwrite');
    final store = txn.objectStore(itemsStore);
    final index = store.index('by_item');
    final keys = await index.getAllKeys([ref.type, ref.uuid]);
    for (final k in keys) {
      await store.delete(k);
    }
    await txn.completed;
  }

  @override
  Future<int> countItems(String collectionUuid) async {
    final items = await getItems(collectionUuid);
    return items.length;
  }
}
