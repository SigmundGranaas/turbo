import 'package:sqflite/sqflite.dart';
import 'package:turbo/core/data/database_provider.dart';

import '../models/collection.dart';
import '../models/collection_item_ref.dart';
import 'collection_data_store.dart';

class SQLiteCollectionDataStore implements CollectionDataStore {
  final Database db;

  SQLiteCollectionDataStore(this.db);

  @override
  Future<void> init() async {}

  @override
  Future<List<Collection>> getAll() async {
    final maps = await db.query(
      collectionsTable,
      orderBy: 'sort_order ASC, created_at ASC',
    );
    return maps.map(Collection.fromLocalMap).toList();
  }

  @override
  Future<Collection?> getByUuid(String uuid) async {
    final maps = await db.query(
      collectionsTable,
      where: 'uuid = ?',
      whereArgs: [uuid],
      limit: 1,
    );
    if (maps.isEmpty) return null;
    return Collection.fromLocalMap(maps.first);
  }

  @override
  Future<void> insert(Collection collection) async {
    await db.insert(
      collectionsTable,
      collection.toLocalMap(),
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  @override
  Future<void> update(Collection collection) async {
    await db.update(
      collectionsTable,
      collection.toLocalMap(),
      where: 'uuid = ?',
      whereArgs: [collection.uuid],
    );
  }

  @override
  Future<void> delete(String uuid) async {
    await db.transaction((txn) async {
      await txn.delete(
        collectionItemsTable,
        where: 'collection_uuid = ?',
        whereArgs: [uuid],
      );
      await txn.delete(
        collectionsTable,
        where: 'uuid = ?',
        whereArgs: [uuid],
      );
    });
  }

  @override
  Future<List<CollectionItemRef>> getItems(String collectionUuid) async {
    final maps = await db.query(
      collectionItemsTable,
      where: 'collection_uuid = ?',
      whereArgs: [collectionUuid],
      orderBy: 'added_at ASC',
    );
    return maps
        .map((m) => CollectionItemRef(
              type: m['item_type'] as String,
              uuid: m['item_uuid'] as String,
            ))
        .toList();
  }

  @override
  Future<List<String>> getCollectionUuidsFor(CollectionItemRef ref) async {
    final maps = await db.query(
      collectionItemsTable,
      columns: ['collection_uuid'],
      where: 'item_type = ? AND item_uuid = ?',
      whereArgs: [ref.type, ref.uuid],
    );
    return maps.map((m) => m['collection_uuid'] as String).toList();
  }

  @override
  Future<Map<CollectionItemRef, List<String>>> getMembershipIndex() async {
    final maps = await db.query(collectionItemsTable);
    final index = <CollectionItemRef, List<String>>{};
    for (final m in maps) {
      final ref = CollectionItemRef(
        type: m['item_type'] as String,
        uuid: m['item_uuid'] as String,
      );
      (index[ref] ??= <String>[]).add(m['collection_uuid'] as String);
    }
    return index;
  }

  @override
  Future<void> addItem(String collectionUuid, CollectionItemRef ref) async {
    await db.insert(
      collectionItemsTable,
      {
        'collection_uuid': collectionUuid,
        'item_type': ref.type,
        'item_uuid': ref.uuid,
        'added_at': DateTime.now().toIso8601String(),
      },
      conflictAlgorithm: ConflictAlgorithm.ignore,
    );
  }

  @override
  Future<void> removeItem(String collectionUuid, CollectionItemRef ref) async {
    await db.delete(
      collectionItemsTable,
      where: 'collection_uuid = ? AND item_type = ? AND item_uuid = ?',
      whereArgs: [collectionUuid, ref.type, ref.uuid],
    );
  }

  @override
  Future<void> removeItemFromAll(CollectionItemRef ref) async {
    await db.delete(
      collectionItemsTable,
      where: 'item_type = ? AND item_uuid = ?',
      whereArgs: [ref.type, ref.uuid],
    );
  }

  @override
  Future<int> countItems(String collectionUuid) async {
    final result = await db.rawQuery(
      'SELECT COUNT(*) AS c FROM $collectionItemsTable WHERE collection_uuid = ?',
      [collectionUuid],
    );
    return (result.first['c'] as num).toInt();
  }
}
