import 'dart:io';

import 'package:path/path.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

import '../../model/marker.dart';
import '../marker_data_store.dart';

class SQLiteMarkerDataStore implements MarkerDataStore {
  Database? _db;

  void injectDatabase(Database database) {
    _db = database;
  }

  @override
  Future<void> init() async {
    if (Platform.isLinux || Platform.isWindows || Platform.isMacOS) {
      sqfliteFfiInit();
      databaseFactory = databaseFactoryFfi;
    }
    _db = await openDatabase(
      join(await getDatabasesPath(), 'markers.db'),
      onCreate: (db, version) async {
        await createTable(db);
      },
      onOpen: (db) async {
        // Ensure the table exists even if the database was already created
        await createTable(db);
      },
      version: 1,
    );
  }

  Future<void> createTable(Database db) async {
    //await db.execute('DROP TABLE markers');
    await db.execute(
      'CREATE TABLE IF NOT EXISTS markers(uuid TEXT PRIMARY KEY, latitude REAL, longitude REAL, title TEXT, description TEXT, icon TEXT, synced INTEGER)',
    );
  }


  @override
  Future<void> insert(Marker marker) async {
    await _db!.insert(
      'markers',
      marker.toMap(),
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  @override
  Future<Marker?> getByUuid(String uuid) async {
    final List<Map<String, dynamic>> maps = await _db!.query(
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
  Future<List<Marker>> findByName(String name) async {
    final List<Map<String, dynamic>> maps = await _db!.query(
      'markers',
      where: 'name LIKE ?',
      whereArgs: [name],
    );

    if (maps.isNotEmpty) {
      return maps.map((el) => Marker.fromMap(el)).toList();
    }
    return List.empty();
  }

  @override
  Future<List<Marker>> getAll() async {
    // First, debug what's in the database
    final List<Map<String, dynamic>> maps = await _db!.query('markers');

    if (maps.isNotEmpty) {
      print("Database contains ${maps.length} markers");
      print("First marker: ${maps.first}");
    }

    // Convert to Marker objects
    final markers = List.generate(maps.length, (i) {
      try {
        return Marker.fromMap(maps[i]);
      } catch (e) {
        print("Error converting marker at index $i: $e");
        print("Data: ${maps[i]}");
        // Return a default marker or null to be filtered later
        return null;
      }
    }).whereType<Marker>().toList(); // Filter out nulls

    print("Converted ${markers.length} valid markers");
    return markers;
  }

  @override
  Future<void> update(Marker marker) async {
    await _db!.update(
      'markers',
      marker.toMap(),
      where: 'uuid = ?',
      whereArgs: [marker.uuid],
    );
  }

  @override
  Future<void> delete(String uuid) async {
    await _db!.delete(
      'markers',
      where: 'uuid = ?',
      whereArgs: [uuid],
    );
  }
}