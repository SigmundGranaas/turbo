import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

const String _dbName = 'turbo_app_v1.db';
const int _dbVersion = 4;

// Table Names
const String regionsTable = 'offline_regions';
const String tileJobsTable = 'tile_jobs';
const String tileStoreTable = 'tile_store';
const String markersTable = 'markers';
const String pendingDeletesTable = 'pending_deletes';
const String savedPathsTable = 'saved_paths';


/// A provider that creates and holds the single instance of the app's database.
/// Other providers will depend on this to get their database connection.
final databaseProvider = FutureProvider<Database>((ref) async {
  // Initialize FFI for desktop platforms if necessary
  if (!kIsWeb && (Platform.isLinux || Platform.isWindows || Platform.isMacOS)) {
    sqfliteFfiInit();
    databaseFactory = databaseFactoryFfi;
  }

  return openDatabase(
    join(await getDatabasesPath(), _dbName),
    version: _dbVersion,
    onCreate: _createDb,
    onUpgrade: _upgradeDb,
  );
});


Future<void> _createDb(Database db, int version) async {
  final batch = db.batch();

  // From SQLiteMarkerDataStore
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
  batch.execute('CREATE INDEX idx_markers_coords ON $markersTable(latitude, longitude)');
  batch.execute('CREATE INDEX idx_markers_synced ON $markersTable(synced)');

  // From RegionRepository
  batch.execute('''
    CREATE TABLE $regionsTable(
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      minLat REAL NOT NULL,
      minLng REAL NOT NULL,
      maxLat REAL NOT NULL,
      maxLng REAL NOT NULL,
      minZoom INTEGER NOT NULL,
      maxZoom INTEGER NOT NULL,
      urlTemplate TEXT NOT NULL,
      tileProviderId TEXT NOT NULL,
      tileProviderName TEXT NOT NULL,
      status INTEGER NOT NULL,
      totalTiles INTEGER NOT NULL,
      downloadedTiles INTEGER NOT NULL,
      createdAt TEXT NOT NULL
    )
  ''');

  // From TileJobQueue
  batch.execute('''
    CREATE TABLE $tileJobsTable(
      regionId TEXT NOT NULL,
      providerId TEXT NOT NULL,
      z INTEGER NOT NULL,
      x INTEGER NOT NULL,
      y INTEGER NOT NULL,
      url TEXT NOT NULL,
      status INTEGER NOT NULL,
      attemptCount INTEGER NOT NULL DEFAULT 0,
      workerId TEXT,
      startedAt TEXT,
      PRIMARY KEY (regionId, z, x, y)
    )
  ''');
  batch.execute('CREATE INDEX idx_job_status ON $tileJobsTable (status)');

  // From TileStoreService
  batch.execute('''
    CREATE TABLE $tileStoreTable(
      providerId TEXT NOT NULL,
      z INTEGER NOT NULL,
      x INTEGER NOT NULL,
      y INTEGER NOT NULL,
      path TEXT NOT NULL,
      sizeInBytes INTEGER NOT NULL,
      lastAccessed TEXT NOT NULL,
      referenceCount INTEGER NOT NULL DEFAULT 0,
      PRIMARY KEY (providerId, z, x, y)
    )
  ''');
  batch.execute('CREATE INDEX idx_tile_path ON $tileStoreTable (path)');
  batch.execute('CREATE INDEX idx_tile_ref_count ON $tileStoreTable (referenceCount)');

  // Pending deletes queue (v2)
  batch.execute('''
    CREATE TABLE $pendingDeletesTable(
      uuid TEXT PRIMARY KEY,
      created_at TEXT NOT NULL
    )
  ''');

  // Saved paths (v3, extended in v4)
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
  batch.execute('CREATE INDEX idx_saved_paths_bounds ON $savedPathsTable(min_lat, max_lat, min_lng, max_lng)');

  await batch.commit(noResult: true);
}

Future<void> _upgradeDb(Database db, int oldVersion, int newVersion) async {
  for (var version = oldVersion + 1; version <= newVersion; version++) {
    switch (version) {
      case 2:
        await _migrateV1ToV2(db);
      case 3:
        await _migrateV2ToV3(db);
      case 4:
        await _migrateV3ToV4(db);
    }
  }
}

Future<void> _migrateV3ToV4(Database db) async {
  await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN color_hex TEXT');
  await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN icon_key TEXT');
  await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN smoothing INTEGER NOT NULL DEFAULT 0');
  await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN line_style TEXT');
}

Future<void> _migrateV1ToV2(Database db) async {
  await db.execute('''
    CREATE TABLE $pendingDeletesTable(
      uuid TEXT PRIMARY KEY,
      created_at TEXT NOT NULL
    )
  ''');
}

Future<void> _migrateV2ToV3(Database db) async {
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
      created_at TEXT NOT NULL
    )
  ''');
  await db.execute('CREATE INDEX idx_saved_paths_bounds ON $savedPathsTable(min_lat, max_lat, min_lng, max_lng)');
}