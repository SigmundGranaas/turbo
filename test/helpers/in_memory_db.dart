import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/core/data/database_provider.dart';

import 'pump_app.dart';

/// Opens a fresh in-memory SQLite database and creates the saved-paths schema
/// matching the production DDL at `lib/core/data/database_provider.dart`.
///
/// Use in `setUp` and remember to `await db.close()` in `tearDown`.
Future<Database> createSavedPathsDb() async {
  initSqfliteFfi();
  final db = await databaseFactory.openDatabase(inMemoryDatabasePath);
  await db.execute('''
    CREATE TABLE $savedPathsTable(
      uuid TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      description TEXT,
      points TEXT NOT NULL,
      distance REAL NOT NULL,
      min_lat REAL NOT NULL,
      min_lng REAL NOT NULL,
      max_lat REAL NOT NULL,
      max_lng REAL NOT NULL,
      created_at TEXT NOT NULL,
      color_hex TEXT,
      icon_key TEXT,
      smoothing INTEGER NOT NULL DEFAULT 0,
      line_style TEXT
    )
  ''');
  await db.execute(
      'CREATE INDEX idx_saved_paths_bounds ON $savedPathsTable(min_lat, max_lat, min_lng, max_lng)');
  return db;
}

/// Opens a fresh in-memory SQLite database and creates the markers schema.
Future<Database> createMarkersDb() async {
  initSqfliteFfi();
  final db = await databaseFactory.openDatabase(inMemoryDatabasePath);
  await db.execute('''
    CREATE TABLE $markersTable(
      uuid TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      description TEXT,
      icon TEXT,
      latitude REAL NOT NULL,
      longitude REAL NOT NULL,
      synced INTEGER NOT NULL DEFAULT 0
    )
  ''');
  await db.execute(
      'CREATE INDEX idx_markers_coords ON $markersTable(latitude, longitude)');
  await db.execute(
      'CREATE INDEX idx_markers_synced ON $markersTable(synced)');
  await db.execute('''
    CREATE TABLE $pendingDeletesTable(
      uuid TEXT PRIMARY KEY,
      created_at TEXT NOT NULL
    )
  ''');
  return db;
}

/// Opens a fully-populated in-memory database with every table the app uses.
/// Useful for cross-feature e2e tests (e.g. measuring → save as path while
/// markers are visible).
Future<Database> createFullSchemaDb() async {
  initSqfliteFfi();
  final db = await databaseFactory.openDatabase(inMemoryDatabasePath);
  final batch = db.batch();
  batch.execute('''
    CREATE TABLE $markersTable(
      uuid TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      description TEXT,
      icon TEXT,
      latitude REAL NOT NULL,
      longitude REAL NOT NULL,
      synced INTEGER NOT NULL DEFAULT 0
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_markers_coords ON $markersTable(latitude, longitude)');
  batch.execute(
      'CREATE INDEX idx_markers_synced ON $markersTable(synced)');
  batch.execute('''
    CREATE TABLE $pendingDeletesTable(
      uuid TEXT PRIMARY KEY,
      created_at TEXT NOT NULL
    )
  ''');
  batch.execute('''
    CREATE TABLE $savedPathsTable(
      uuid TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      description TEXT,
      points TEXT NOT NULL,
      distance REAL NOT NULL,
      min_lat REAL NOT NULL,
      min_lng REAL NOT NULL,
      max_lat REAL NOT NULL,
      max_lng REAL NOT NULL,
      created_at TEXT NOT NULL,
      color_hex TEXT,
      icon_key TEXT,
      smoothing INTEGER NOT NULL DEFAULT 0,
      line_style TEXT
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_saved_paths_bounds ON $savedPathsTable(min_lat, max_lat, min_lng, max_lng)');
  await batch.commit(noResult: true);
  return db;
}
