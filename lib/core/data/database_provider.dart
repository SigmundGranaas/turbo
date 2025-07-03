import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

const String _dbName = 'turbo_app_v1.db';
const int _dbVersion = 1;

// Table Names
const String regionsTable = 'offline_regions';
const String tileJobsTable = 'tile_jobs';
const String tileStoreTable = 'tile_store';
const String markersTable = 'markers';


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


  await batch.commit(noResult: true);
}