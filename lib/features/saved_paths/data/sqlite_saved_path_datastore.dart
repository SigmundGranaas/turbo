import 'package:latlong2/latlong.dart';
import 'package:sqflite/sqflite.dart';
import 'package:turbo/core/data/database_provider.dart';
import '../models/saved_path.dart';
import 'saved_path_data_store.dart';

class SQLiteSavedPathDataStore implements SavedPathDataStore {
  final Database db;

  SQLiteSavedPathDataStore(this.db);

  @override
  Future<void> init() async {}

  @override
  Future<void> insert(SavedPath path) async {
    await db.insert(
      savedPathsTable,
      path.toLocalMap(),
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  @override
  Future<SavedPath?> getByUuid(String uuid) async {
    final maps = await db.query(
      savedPathsTable,
      where: 'uuid = ?',
      whereArgs: [uuid],
      limit: 1,
    );
    if (maps.isNotEmpty) {
      return SavedPath.fromLocalMap(maps.first);
    }
    return null;
  }

  @override
  Future<List<SavedPath>> getAll() async {
    final maps = await db.query(savedPathsTable);
    return maps.map((map) => SavedPath.fromLocalMap(map)).toList();
  }

  @override
  Future<void> update(SavedPath path) async {
    await db.update(
      savedPathsTable,
      path.toLocalMap(),
      where: 'uuid = ?',
      whereArgs: [path.uuid],
    );
  }

  @override
  Future<void> delete(String uuid) async {
    await db.delete(
      savedPathsTable,
      where: 'uuid = ?',
      whereArgs: [uuid],
    );
  }

  @override
  Future<List<SavedPath>> findInBounds(LatLng southwest, LatLng northeast) async {
    final maps = await db.query(
      savedPathsTable,
      where: 'min_lat <= ? AND max_lat >= ? AND min_lng <= ? AND max_lng >= ?',
      whereArgs: [
        northeast.latitude,
        southwest.latitude,
        northeast.longitude,
        southwest.longitude,
      ],
    );
    return maps.map((map) => SavedPath.fromLocalMap(map)).toList();
  }
}
