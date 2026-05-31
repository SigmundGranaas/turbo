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
      moving_time_seconds INTEGER,
      synced INTEGER NOT NULL DEFAULT 0,
      version INTEGER,
      updated_at TEXT,
      deleted_at TEXT
    )
  ''');
  await db.execute(
      'CREATE INDEX idx_saved_paths_bounds ON $savedPathsTable(min_lat, max_lat, min_lng, max_lng)');
  await db.execute(
      'CREATE INDEX idx_saved_paths_updated_at ON $savedPathsTable(updated_at)');
  return db;
}

/// Opens a fresh in-memory SQLite database with the activities offline
/// schema (v10): activity_summaries + activity_conditions_cache +
/// activity_details_cache. Mirrors the DDL in
/// `lib/core/data/database_provider.dart`.
Future<Database> createActivitiesDb() async {
  initSqfliteFfi();
  final db = await databaseFactory.openDatabase(inMemoryDatabasePath);
  final batch = db.batch();
  batch.execute('''
    CREATE TABLE $activitySummariesTable(
      id TEXT PRIMARY KEY,
      kind TEXT NOT NULL,
      name TEXT NOT NULL,
      geometry_wkt TEXT NOT NULL,
      geometry_kind TEXT NOT NULL,
      icon_key TEXT NOT NULL,
      color_hex TEXT,
      updated_at INTEGER NOT NULL,
      version INTEGER NOT NULL,
      summary_score INTEGER,
      summary_score_at INTEGER,
      top_driver_label TEXT
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_activity_summaries_kind ON $activitySummariesTable(kind)');
  batch.execute(
      'CREATE INDEX idx_activity_summaries_updated ON $activitySummariesTable(updated_at)');
  batch.execute('''
    CREATE TABLE $activityConditionsCacheTable(
      activity_id TEXT NOT NULL,
      kind TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      fetched_at INTEGER NOT NULL,
      PRIMARY KEY (activity_id, kind)
    )
  ''');
  batch.execute('''
    CREATE TABLE $activityDetailsCacheTable(
      activity_id TEXT NOT NULL,
      kind TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      fetched_at INTEGER NOT NULL,
      PRIMARY KEY (activity_id, kind)
    )
  ''');
  await batch.commit(noResult: true);
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
      synced INTEGER NOT NULL DEFAULT 0,
      version INTEGER,
      updated_at TEXT,
      deleted_at TEXT
    )
  ''');
  await db.execute(
      'CREATE INDEX idx_markers_coords ON $markersTable(latitude, longitude)');
  await db.execute(
      'CREATE INDEX idx_markers_synced ON $markersTable(synced)');
  await db.execute(
      'CREATE INDEX idx_markers_updated_at ON $markersTable(updated_at)');
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
      synced INTEGER NOT NULL DEFAULT 0,
      version INTEGER,
      updated_at TEXT,
      deleted_at TEXT
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_markers_coords ON $markersTable(latitude, longitude)');
  batch.execute(
      'CREATE INDEX idx_markers_synced ON $markersTable(synced)');
  batch.execute(
      'CREATE INDEX idx_markers_updated_at ON $markersTable(updated_at)');
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
      moving_time_seconds INTEGER,
      synced INTEGER NOT NULL DEFAULT 0,
      version INTEGER,
      updated_at TEXT,
      deleted_at TEXT
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_saved_paths_bounds ON $savedPathsTable(min_lat, max_lat, min_lng, max_lng)');
  batch.execute(
      'CREATE INDEX idx_saved_paths_updated_at ON $savedPathsTable(updated_at)');
  batch.execute('''
    CREATE TABLE $collectionsTable(
      uuid TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      description TEXT,
      color_hex TEXT,
      icon_key TEXT,
      created_at TEXT NOT NULL,
      sort_order INTEGER NOT NULL DEFAULT 0,
      saved_filter TEXT,
      synced INTEGER NOT NULL DEFAULT 0,
      version INTEGER,
      updated_at TEXT,
      deleted_at TEXT
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_collections_updated_at ON $collectionsTable(updated_at)');
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
      sort_order INTEGER NOT NULL DEFAULT 0,
      saved_filter TEXT,
      synced INTEGER NOT NULL DEFAULT 0,
      version INTEGER,
      updated_at TEXT,
      deleted_at TEXT
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_collections_updated_at ON $collectionsTable(updated_at)');
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
