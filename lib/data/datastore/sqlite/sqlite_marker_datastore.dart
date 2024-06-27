import 'dart:io';

import 'package:path/path.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

import '../../model/marker.dart';
import '../marker_data_store.dart';

class SQLiteMarkerDataStore implements MarkerDataStore {
  late Database _db;

  @override
  Future<void> init() async {
    if (Platform.isLinux || Platform.isWindows || Platform.isMacOS) {
      // Initialize FFI for desktop platforms
      sqfliteFfiInit();
      databaseFactory = databaseFactoryFfi;
    }
    _db = await openDatabase(
      join(await getDatabasesPath(), 'markers.db'),
      onCreate: (db, version) {
        return db.execute(
          'CREATE TABLE markers(uuid TEXT PRIMARY KEY, latitude REAL, longitude REAL, title TEXT, description TEXT, icon TEXT)',
        );
      },
      version: 1,
    );
  }

  @override
  Future<void> insert(Marker marker) async {
    await _db.insert(
      'markers',
      marker.toMap(),
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  @override
  Future<Marker?> getByUuid(String uuid) async {
    final List<Map<String, dynamic>> maps = await _db.query(
      'markers',
      where: 'uuid = ?',
      whereArgs: [uuid],
    );

    if (maps.isNotEmpty) {
      return Marker.fromMap(maps.first);
    }
    return null;
  }

  @override
  Future<List<Marker>> getAll() async {
    final List<Map<String, dynamic>> maps = await _db.query('markers');
    return List.generate(maps.length, (i) => Marker.fromMap(maps[i]));
  }

  @override
  Future<void> update(Marker marker) async {
    await _db.update(
      'markers',
      marker.toMap(),
      where: 'uuid = ?',
      whereArgs: [marker.uuid],
    );
  }

  @override
  Future<void> delete(String uuid) async {
    await _db.delete(
      'markers',
      where: 'uuid = ?',
      whereArgs: [uuid],
    );
  }
}