import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:sqflite/sqflite.dart';
import 'package:turbo/core/data/database_provider.dart';

final pendingDeleteStoreProvider = FutureProvider<PendingDeleteStore>((ref) async {
  if (kIsWeb) return const _NoOpPendingDeleteStore();
  final db = await ref.watch(databaseProvider.future);
  return SqlitePendingDeleteStore(db);
});

abstract class PendingDeleteStore {
  Future<void> add(String uuid);
  Future<void> remove(String uuid);
  Future<List<String>> getAll();
}

class SqlitePendingDeleteStore implements PendingDeleteStore {
  final Database _db;

  SqlitePendingDeleteStore(this._db);

  @override
  Future<void> add(String uuid) async {
    await _db.insert(
      pendingDeletesTable,
      {'uuid': uuid, 'created_at': DateTime.now().toIso8601String()},
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  @override
  Future<void> remove(String uuid) async {
    await _db.delete(
      pendingDeletesTable,
      where: 'uuid = ?',
      whereArgs: [uuid],
    );
  }

  @override
  Future<List<String>> getAll() async {
    final maps = await _db.query(pendingDeletesTable);
    return maps.map((map) => map['uuid'] as String).toList();
  }
}

class _NoOpPendingDeleteStore implements PendingDeleteStore {
  const _NoOpPendingDeleteStore();

  @override
  Future<void> add(String uuid) async {}

  @override
  Future<void> remove(String uuid) async {}

  @override
  Future<List<String>> getAll() async => [];
}
