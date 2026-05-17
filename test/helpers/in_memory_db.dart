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
      line_style TEXT,
      elevations TEXT,
      recorded_at TEXT,
      ascent REAL,
      descent REAL,
      moving_time_seconds INTEGER
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
  await db.execute('''
    CREATE TABLE $markerPhotosTable(
      uuid TEXT PRIMARY KEY,
      marker_uuid TEXT NOT NULL,
      file_path TEXT NOT NULL,
      created_at TEXT NOT NULL
    )
  ''');
  await db.execute(
      'CREATE INDEX idx_marker_photos_marker ON $markerPhotosTable(marker_uuid)');
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
      line_style TEXT,
      elevations TEXT,
      recorded_at TEXT,
      ascent REAL,
      descent REAL,
      moving_time_seconds INTEGER
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_saved_paths_bounds ON $savedPathsTable(min_lat, max_lat, min_lng, max_lng)');
  batch.execute('''
    CREATE TABLE $collectionsTable(
      uuid TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      description TEXT,
      color_hex TEXT,
      icon_key TEXT,
      created_at TEXT NOT NULL,
      sort_order INTEGER NOT NULL DEFAULT 0
    )
  ''');
  batch.execute('''
    CREATE TABLE $collectionItemsTable(
      collection_uuid TEXT NOT NULL,
      item_type TEXT NOT NULL,
      item_uuid TEXT NOT NULL,
      added_at TEXT NOT NULL,
      PRIMARY KEY (collection_uuid, item_type, item_uuid)
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_collection_items_item ON $collectionItemsTable(item_type, item_uuid)');
  await batch.commit(noResult: true);
  return db;
}

/// Opens a fresh in-memory SQLite database with only the collections schema.
Future<Database> createCollectionsDb() async {
  initSqfliteFfi();
  final db = await databaseFactory.openDatabase(inMemoryDatabasePath);
  final batch = db.batch();
  batch.execute('''
    CREATE TABLE $collectionsTable(
      uuid TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      description TEXT,
      color_hex TEXT,
      icon_key TEXT,
      created_at TEXT NOT NULL,
      sort_order INTEGER NOT NULL DEFAULT 0
    )
  ''');
  batch.execute('''
    CREATE TABLE $collectionItemsTable(
      collection_uuid TEXT NOT NULL,
      item_type TEXT NOT NULL,
      item_uuid TEXT NOT NULL,
      added_at TEXT NOT NULL,
      PRIMARY KEY (collection_uuid, item_type, item_uuid)
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_collection_items_item ON $collectionItemsTable(item_type, item_uuid)');
  await batch.commit(noResult: true);
  return db;
}
