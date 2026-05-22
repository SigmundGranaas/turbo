import 'package:latlong2/latlong.dart';
import 'package:sqflite/sqflite.dart';
import 'package:turbo/core/data/database_provider.dart';
import '../models/marker.dart';
import 'marker_data_store.dart';

class SQLiteMarkerDataStore implements MarkerDataStore {
  final Database db;

  SQLiteMarkerDataStore(this.db);

  @override
  Future<void> init() async {
  }

  @override
  Future<void> insert(Marker marker) async {
    await db.insert(
      markersTable,
      marker.toLocalMap(),
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  @override
  Future<Marker?> getByUuid(String uuid) async {
    final maps = await db.query(
      markersTable,
      where: 'uuid = ?',
      whereArgs: [uuid],
      limit: 1,
    );
    if (maps.isNotEmpty) {
      return Marker.fromLocalMap(maps.first);
    }
    return null;
  }

  @override
  Future<List<Marker>> getAll() async {
    final maps = await db.query(markersTable);
    return maps.map((map) => Marker.fromLocalMap(map)).toList();
  }

  @override
  Future<List<Marker>> getUnsynced() async {
    final maps = await db.query(markersTable, where: 'synced = ?', whereArgs: [0]);
    return maps.map((map) => Marker.fromLocalMap(map)).toList();
  }

  @override
  Future<void> update(Marker marker) async {
    await db.update(
      markersTable,
      marker.toLocalMap(),
      where: 'uuid = ?',
      whereArgs: [marker.uuid],
    );
  }

  @override
  Future<void> delete(String uuid) async {
    await db.delete(
      markersTable,
      where: 'uuid = ?',
      whereArgs: [uuid],
    );
  }

  @override
  Future<void> deleteAll(List<String> uuids) async {
    if (uuids.isEmpty) return;
    final batch = db.batch();
    for (final uuid in uuids) {
      batch.delete(markersTable, where: 'uuid = ?', whereArgs: [uuid]);
    }
    await batch.commit(noResult: true);
  }

  @override
  Future<void> clearAll() async {
    await db.delete(markersTable);
  }

  @override
  Future<List<Marker>> findInBounds(LatLng southwest, LatLng northeast) async {
    final maps = await db.query(
      markersTable,
      where: 'latitude >= ? AND latitude <= ? AND longitude >= ? AND longitude <= ?',
      whereArgs: [
        southwest.latitude,
        northeast.latitude,
        southwest.longitude,
        northeast.longitude,
      ],
    );
    return maps.map((map) => Marker.fromLocalMap(map)).toList();
  }

}