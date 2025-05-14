import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:latlong2/latlong.dart';
import 'package:path/path.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import '../../model/marker.dart';
import '../marker_data_store.dart';

class SQLiteMarkerDataStore implements MarkerDataStore {
  Database? _db;
  static const String _tableName = 'markers';

  Future<Database> get database async {
    if (_db != null) return _db!;
    _db = await _initDB();
    return _db!;
  }

  Future<Database> _initDB() async {
    if (!kIsWeb && (Platform.isLinux || Platform.isWindows || Platform.isMacOS)) {
      sqfliteFfiInit();
      databaseFactory = databaseFactoryFfi;
    }
    return openDatabase(
      join(await getDatabasesPath(), 'markers_v2.db'),
      version: 1,
      onCreate: (db, version) async {
        await db.execute('''
          CREATE TABLE $_tableName(
            uuid TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            icon TEXT,
            latitude REAL NOT NULL,
            longitude REAL NOT NULL,
            synced INTEGER NOT NULL DEFAULT 0
          )
        ''');
        await db.execute('CREATE INDEX idx_markers_coords ON $_tableName(latitude, longitude)');
        await db.execute('CREATE INDEX idx_markers_synced ON $_tableName(synced)');
      },
    );
  }

  @override
  Future<void> init() async {
    await database; // Ensures DB is initialized
  }

  @override
  Future<void> insert(Marker marker) async {
    final db = await database;
    await db.insert(
      _tableName,
      marker.toLocalMap(),
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  @override
  Future<Marker?> getByUuid(String uuid) async {
    final db = await database;
    final maps = await db.query(
      _tableName,
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
    final db = await database;
    final maps = await db.query(_tableName);
    return maps.map((map) => Marker.fromLocalMap(map)).toList();
  }

  @override
  Future<List<Marker>> getUnsynced() async {
    final db = await database;
    final maps = await db.query(_tableName, where: 'synced = ?', whereArgs: [0]);
    return maps.map((map) => Marker.fromLocalMap(map)).toList();
  }

  @override
  Future<void> update(Marker marker) async {
    final db = await database;
    await db.update(
      _tableName,
      marker.toLocalMap(),
      where: 'uuid = ?',
      whereArgs: [marker.uuid],
    );
  }

  @override
  Future<void> delete(String uuid) async {
    final db = await database;
    await db.delete(
      _tableName,
      where: 'uuid = ?',
      whereArgs: [uuid],
    );
  }

  @override
  Future<void> deleteAll(List<String> uuids) async {
    if (uuids.isEmpty) return;
    final db = await database;
    final batch = db.batch();
    for (final uuid in uuids) {
      batch.delete(_tableName, where: 'uuid = ?', whereArgs: [uuid]);
    }
    await batch.commit(noResult: true);
  }


  @override
  Future<void> clearAll() async {
    final db = await database;
    await db.delete(_tableName);
  }

  @override
  Future<List<Marker>> findInBounds(LatLng southwest, LatLng northeast) async {
    final db = await database;
    final maps = await db.query(
      _tableName,
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