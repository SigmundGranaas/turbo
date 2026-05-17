import 'package:sqflite/sqflite.dart';
import 'package:turbo/core/data/database_provider.dart';

import '../models/marker_photo.dart';
import 'marker_photo_data_store.dart';

class SQLiteMarkerPhotoDataStore implements MarkerPhotoDataStore {
  final Database db;

  SQLiteMarkerPhotoDataStore(this.db);

  @override
  Future<void> init() async {}

  @override
  Future<void> insert(MarkerPhoto photo) async {
    await db.insert(
      markerPhotosTable,
      photo.toLocalMap(),
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  @override
  Future<List<MarkerPhoto>> getByMarker(String markerUuid) async {
    final maps = await db.query(
      markerPhotosTable,
      where: 'marker_uuid = ?',
      whereArgs: [markerUuid],
      orderBy: 'created_at ASC',
    );
    return maps.map(MarkerPhoto.fromLocalMap).toList();
  }

  @override
  Future<MarkerPhoto?> getByUuid(String uuid) async {
    final maps = await db.query(
      markerPhotosTable,
      where: 'uuid = ?',
      whereArgs: [uuid],
      limit: 1,
    );
    if (maps.isEmpty) return null;
    return MarkerPhoto.fromLocalMap(maps.first);
  }

  @override
  Future<void> delete(String uuid) async {
    await db.delete(markerPhotosTable, where: 'uuid = ?', whereArgs: [uuid]);
  }

  @override
  Future<void> deleteAllForMarker(String markerUuid) async {
    await db.delete(markerPhotosTable,
        where: 'marker_uuid = ?', whereArgs: [markerUuid]);
  }
}
