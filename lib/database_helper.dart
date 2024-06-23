import 'dart:io';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:path/path.dart';
import 'package:path_provider/path_provider.dart';

class DatabaseHelper {
  static Database? _database;

  static Future<void> initDatabase() async {
    if (Platform.isLinux || Platform.isWindows || Platform.isMacOS) {
      // Initialize FFI for desktop platforms
      sqfliteFfiInit();
      databaseFactory = databaseFactoryFfi;
    }
  }

  static Future<Database> get database async {
    if (_database != null) return _database!;
    _database = await _initDb();
    return _database!;
  }

  static Future<Database> _initDb() async {
    Directory documentsDirectory = await getApplicationDocumentsDirectory();
    String path = join(documentsDirectory.path, "locations.db");
    return await openDatabase(path, version: 1, onCreate: _onCreate);
  }

  static Future _onCreate(Database db, int version) async {
    await db.execute('''
      CREATE TABLE locations (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT,
        description TEXT,
        latitude REAL,
        longitude REAL
      )
    ''');
  }

  static Future<int> insertLocation(Map<String, dynamic> row) async {
    Database db = await database;
    return await db.insert('locations', row);
  }

  static Future<List<Map<String, dynamic>>> queryAllLocations() async {
    Database db = await database;
    return await db.query('locations');
  }

  static Future<int> updateLocation(Map<String, dynamic> row) async {
    Database db = await database;
    return await db.update('locations', row, where: 'id = ?', whereArgs: [row['id']]);
  }

  static Future<int> deleteLocation(int id) async {
    Database db = await database;
    return await db.delete('locations', where: 'id = ?', whereArgs: [id]);
  }
}