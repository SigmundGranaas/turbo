import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

const String _dbName = 'turbo_app_v1.db';
const int _dbVersion = 14;

// Table Names
const String regionsTable = 'offline_regions';
const String tileJobsTable = 'tile_jobs';
const String tileStoreTable = 'tile_store';
const String markersTable = 'markers';
const String pendingDeletesTable = 'pending_deletes';
const String savedPathsTable = 'saved_paths';
const String collectionsTable = 'collections';
const String collectionItemsTable = 'collection_items';
const String markerPhotosTable = 'marker_photos';
const String vectorTileCacheTable = 'vector_tile_cache';
const String activitySummariesTable = 'activity_summaries';
const String activityConditionsCacheTable = 'activity_conditions_cache';
const String activityDetailsCacheTable = 'activity_details_cache';
const String activityAnalysisCacheTable = 'activity_analysis_cache';


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

  // From SQLiteMarkerDataStore — extended in v10 with sync columns
  // (version/updated_at/deleted_at) so the client can drive delta-sync
  // and optimistic-concurrency against the server.
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
  batch.execute('CREATE INDEX idx_markers_coords ON $markersTable(latitude, longitude)');
  batch.execute('CREATE INDEX idx_markers_synced ON $markersTable(synced)');
  batch.execute('CREATE INDEX idx_markers_updated_at ON $markersTable(updated_at)');

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

  // Saved paths (v3, extended in v4, v6, and v10).
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
  batch.execute('CREATE INDEX idx_saved_paths_bounds ON $savedPathsTable(min_lat, max_lat, min_lng, max_lng)');
  batch.execute('CREATE INDEX idx_saved_paths_updated_at ON $savedPathsTable(updated_at)');

  // Pending delete queue for tracks (mirrors pending_deletes for markers).
  batch.execute('''
    CREATE TABLE pending_track_deletes(
      uuid TEXT PRIMARY KEY,
      version INTEGER,
      created_at TEXT NOT NULL
    )
  ''');

  // Collections + join table (v5, extended in v11 with sync columns).
  // The join table is keyed by (type, uuid) so future item types do
  // not require schema changes.
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
  batch.execute('CREATE INDEX idx_collections_updated_at ON $collectionsTable(updated_at)');
  batch.execute('''
    CREATE TABLE $collectionItemsTable(
      collection_uuid TEXT NOT NULL,
      item_type TEXT NOT NULL,
      item_uuid TEXT NOT NULL,
      added_at TEXT NOT NULL,
      PRIMARY KEY (collection_uuid, item_type, item_uuid),
      FOREIGN KEY (collection_uuid) REFERENCES $collectionsTable(uuid) ON DELETE CASCADE
    )
  ''');
  batch.execute('CREATE INDEX idx_collection_items_item ON $collectionItemsTable(item_type, item_uuid)');

  // Marker photos (v7).
  batch.execute('''
    CREATE TABLE $markerPhotosTable(
      uuid TEXT PRIMARY KEY,
      marker_uuid TEXT NOT NULL,
      file_path TEXT NOT NULL,
      created_at TEXT NOT NULL,
      FOREIGN KEY (marker_uuid) REFERENCES $markersTable(uuid) ON DELETE CASCADE
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_marker_photos_marker ON $markerPhotosTable(marker_uuid)');

  // Vector tile cache (v9) — backing store for the external_vector_layers
  // feature. One row per (source, z, x, y) viewport tile.
  batch.execute('''
    CREATE TABLE $vectorTileCacheTable(
      source TEXT NOT NULL,
      z INTEGER NOT NULL,
      x INTEGER NOT NULL,
      y INTEGER NOT NULL,
      geojson TEXT NOT NULL,
      fetched_at INTEGER NOT NULL,
      PRIMARY KEY (source, z, x, y)
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_vector_tile_fetched ON $vectorTileCacheTable(fetched_at)');

  // Activities offline cache (v10).
  //
  // activity_summaries mirrors the cross-kind read model the server
  // exposes at /api/activities/summaries/*. We keep it locally so the
  // map paints pins on cold start before the network responds. Delta
  // sync writes here on success; tombstones remove rows. The
  // delta_cursor row in activity_meta tracks how far we've consumed.
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
      version INTEGER NOT NULL
    )
  ''');
  batch.execute(
      'CREATE INDEX idx_activity_summaries_kind ON $activitySummariesTable(kind)');
  batch.execute(
      'CREATE INDEX idx_activity_summaries_updated ON $activitySummariesTable(updated_at)');

  // activity_conditions_cache stores the last successful per-kind
  // conditions report by (activity_id, kind). Used as a fallback when
  // /conditions fails so the panel keeps rendering with a "fetched at"
  // hint instead of an error.
  batch.execute('''
    CREATE TABLE $activityConditionsCacheTable(
      activity_id TEXT NOT NULL,
      kind TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      fetched_at INTEGER NOT NULL,
      PRIMARY KEY (activity_id, kind)
    )
  ''');

  // activity_details_cache stores the last successful per-kind activity
  // detail payload — i.e. the typed body of /api/activities/{kind}/{id}.
  // Lets the detail screen render the last-known state when the user
  // taps a pin while offline.
  batch.execute('''
    CREATE TABLE $activityDetailsCacheTable(
      activity_id TEXT NOT NULL,
      kind TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      fetched_at INTEGER NOT NULL,
      PRIMARY KEY (activity_id, kind)
    )
  ''');

  // activity_analysis_cache stores the last successful response from
  // /api/activities/{kind}/{id}/analysis. Kept in its own table — rather
  // than reusing the conditions cache — so the legacy /conditions
  // payload and the richer ActivityAnalysis payload can coexist while
  // kinds migrate one at a time.
  batch.execute('''
    CREATE TABLE $activityAnalysisCacheTable(
      activity_id TEXT NOT NULL,
      kind TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      fetched_at INTEGER NOT NULL,
      PRIMARY KEY (activity_id, kind)
    )
  ''');

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
      case 5:
        await _migrateV4ToV5(db);
      case 6:
        await _migrateV5ToV6(db);
      case 7:
        await _migrateV6ToV7(db);
      case 8:
        await _migrateV7ToV8(db);
      case 9:
        await _migrateV8ToV9(db);
      case 10:
        await _migrateV9ToV10(db);
      case 11:
        await _migrateV10ToV11(db);
      case 12:
        await _migrateV11ToV12(db);
      case 13:
        await _migrateV12ToV13(db);
      case 14:
        await _migrateV13ToV14(db);
    }
  }
}

/// v13 — analysis cache: the per-kind orchestrator response from
/// `/api/activities/{kind}/{id}/analysis`. Same shape as the existing
/// conditions/details caches, separate table so the two payload formats
/// can coexist while kinds migrate.
Future<void> _migrateV12ToV13(Database db) async {
  await db.execute('''
    CREATE TABLE IF NOT EXISTS $activityAnalysisCacheTable(
      activity_id TEXT NOT NULL,
      kind TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      fetched_at INTEGER NOT NULL,
      PRIMARY KEY (activity_id, kind)
    )
  ''');
}

/// v14 — score halo plumbing: the summary projection carries the last
/// orchestrator score so the map layer can shade pin halos without an
/// expensive per-pin analysis fetch.
Future<void> _migrateV13ToV14(Database db) async {
  final columns = (await db.rawQuery('PRAGMA table_info($activitySummariesTable)'))
      .map((row) => row['name'] as String)
      .toSet();
  if (!columns.contains('summary_score')) {
    await db.execute('ALTER TABLE $activitySummariesTable ADD COLUMN summary_score INTEGER');
  }
  if (!columns.contains('summary_score_at')) {
    await db.execute('ALTER TABLE $activitySummariesTable ADD COLUMN summary_score_at INTEGER');
  }
  if (!columns.contains('top_driver_label')) {
    await db.execute('ALTER TABLE $activitySummariesTable ADD COLUMN top_driver_label TEXT');
  }
}

/// v12 — activities offline cache: cross-kind summaries, plus per-(id, kind)
/// conditions and detail payload caches. Lets the map paint pins on cold
/// start and keeps the conditions panel useful when the network drops.
Future<void> _migrateV11ToV12(Database db) async {
  await db.execute('''
    CREATE TABLE IF NOT EXISTS $activitySummariesTable(
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
  await db.execute(
      'CREATE INDEX IF NOT EXISTS idx_activity_summaries_kind ON $activitySummariesTable(kind)');
  await db.execute(
      'CREATE INDEX IF NOT EXISTS idx_activity_summaries_updated ON $activitySummariesTable(updated_at)');

  await db.execute('''
    CREATE TABLE IF NOT EXISTS $activityConditionsCacheTable(
      activity_id TEXT NOT NULL,
      kind TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      fetched_at INTEGER NOT NULL,
      PRIMARY KEY (activity_id, kind)
    )
  ''');

  await db.execute('''
    CREATE TABLE IF NOT EXISTS $activityDetailsCacheTable(
      activity_id TEXT NOT NULL,
      kind TEXT NOT NULL,
      payload_json TEXT NOT NULL,
      fetched_at INTEGER NOT NULL,
      PRIMARY KEY (activity_id, kind)
    )
  ''');
}

/// v11 — sync columns on collections so they participate in the
/// delta-sync flow against the server (version + updated_at + deleted_at).
Future<void> _migrateV10ToV11(Database db) async {
  final columns = (await db.rawQuery('PRAGMA table_info($collectionsTable)'))
      .map((row) => row['name'] as String)
      .toSet();
  if (!columns.contains('synced')) {
    await db.execute(
        'ALTER TABLE $collectionsTable ADD COLUMN synced INTEGER NOT NULL DEFAULT 0');
  }
  if (!columns.contains('version')) {
    await db.execute('ALTER TABLE $collectionsTable ADD COLUMN version INTEGER');
  }
  if (!columns.contains('updated_at')) {
    await db.execute('ALTER TABLE $collectionsTable ADD COLUMN updated_at TEXT');
  }
  if (!columns.contains('deleted_at')) {
    await db.execute('ALTER TABLE $collectionsTable ADD COLUMN deleted_at TEXT');
  }
  await db.execute(
      'CREATE INDEX IF NOT EXISTS idx_collections_updated_at ON $collectionsTable(updated_at)');
}

/// v10 — sync columns on markers and saved_paths to drive the delta-sync
/// flow against the server (version + updated_at + deleted_at), plus a
/// pending-deletes queue for tracks that mirrors the markers' one.
Future<void> _migrateV9ToV10(Database db) async {
  final markerCols = (await db.rawQuery('PRAGMA table_info($markersTable)'))
      .map((row) => row['name'] as String)
      .toSet();
  if (!markerCols.contains('version')) {
    await db.execute('ALTER TABLE $markersTable ADD COLUMN version INTEGER');
  }
  if (!markerCols.contains('updated_at')) {
    await db.execute('ALTER TABLE $markersTable ADD COLUMN updated_at TEXT');
  }
  if (!markerCols.contains('deleted_at')) {
    await db.execute('ALTER TABLE $markersTable ADD COLUMN deleted_at TEXT');
  }
  await db.execute(
      'CREATE INDEX IF NOT EXISTS idx_markers_updated_at ON $markersTable(updated_at)');

  final pathCols = (await db.rawQuery('PRAGMA table_info($savedPathsTable)'))
      .map((row) => row['name'] as String)
      .toSet();
  if (!pathCols.contains('synced')) {
    await db.execute(
        'ALTER TABLE $savedPathsTable ADD COLUMN synced INTEGER NOT NULL DEFAULT 0');
  }
  if (!pathCols.contains('version')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN version INTEGER');
  }
  if (!pathCols.contains('updated_at')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN updated_at TEXT');
  }
  if (!pathCols.contains('deleted_at')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN deleted_at TEXT');
  }
  await db.execute(
      'CREATE INDEX IF NOT EXISTS idx_saved_paths_updated_at ON $savedPathsTable(updated_at)');

  await db.execute('''
    CREATE TABLE IF NOT EXISTS pending_track_deletes(
      uuid TEXT PRIMARY KEY,
      version INTEGER,
      created_at TEXT NOT NULL
    )
  ''');
}

/// v9 — vector tile cache table backing the external_vector_layers feature.
Future<void> _migrateV8ToV9(Database db) async {
  await db.execute('''
    CREATE TABLE IF NOT EXISTS $vectorTileCacheTable(
      source TEXT NOT NULL,
      z INTEGER NOT NULL,
      x INTEGER NOT NULL,
      y INTEGER NOT NULL,
      geojson TEXT NOT NULL,
      fetched_at INTEGER NOT NULL,
      PRIMARY KEY (source, z, x, y)
    )
  ''');
  await db.execute(
      'CREATE INDEX IF NOT EXISTS idx_vector_tile_fetched ON $vectorTileCacheTable(fetched_at)');
}

Future<void> _migrateV7ToV8(Database db) async {
  final columns = (await db.rawQuery('PRAGMA table_info($collectionsTable)'))
      .map((row) => row['name'] as String)
      .toSet();
  if (!columns.contains('saved_filter')) {
    await db.execute(
        'ALTER TABLE $collectionsTable ADD COLUMN saved_filter TEXT');
  }
}

Future<void> _migrateV6ToV7(Database db) async {
  // Idempotent — sqlite ignores CREATE TABLE IF NOT EXISTS on existing tables.
  await db.execute('''
    CREATE TABLE IF NOT EXISTS $markerPhotosTable(
      uuid TEXT PRIMARY KEY,
      marker_uuid TEXT NOT NULL,
      file_path TEXT NOT NULL,
      created_at TEXT NOT NULL,
      FOREIGN KEY (marker_uuid) REFERENCES $markersTable(uuid) ON DELETE CASCADE
    )
  ''');
  await db.execute(
      'CREATE INDEX IF NOT EXISTS idx_marker_photos_marker ON $markerPhotosTable(marker_uuid)');
}

Future<void> _migrateV5ToV6(Database db) async {
  final columns = (await db.rawQuery('PRAGMA table_info($savedPathsTable)'))
      .map((row) => row['name'] as String)
      .toSet();
  if (!columns.contains('elevations')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN elevations TEXT');
  }
  if (!columns.contains('recorded_at')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN recorded_at TEXT');
  }
  if (!columns.contains('ascent')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN ascent REAL');
  }
  if (!columns.contains('descent')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN descent REAL');
  }
  if (!columns.contains('moving_time_seconds')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN moving_time_seconds INTEGER');
  }
}

Future<void> _migrateV4ToV5(Database db) async {
  await db.execute('''
    CREATE TABLE IF NOT EXISTS $collectionsTable(
      uuid TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      description TEXT,
      color_hex TEXT,
      icon_key TEXT,
      created_at TEXT NOT NULL,
      sort_order INTEGER NOT NULL DEFAULT 0
    )
  ''');
  await db.execute('''
    CREATE TABLE IF NOT EXISTS $collectionItemsTable(
      collection_uuid TEXT NOT NULL,
      item_type TEXT NOT NULL,
      item_uuid TEXT NOT NULL,
      added_at TEXT NOT NULL,
      PRIMARY KEY (collection_uuid, item_type, item_uuid),
      FOREIGN KEY (collection_uuid) REFERENCES $collectionsTable(uuid) ON DELETE CASCADE
    )
  ''');
  await db.execute('CREATE INDEX IF NOT EXISTS idx_collection_items_item ON $collectionItemsTable(item_type, item_uuid)');
}

Future<void> _migrateV3ToV4(Database db) async {
  final columns = (await db.rawQuery('PRAGMA table_info($savedPathsTable)'))
      .map((row) => row['name'] as String)
      .toSet();
  if (!columns.contains('color_hex')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN color_hex TEXT');
  }
  if (!columns.contains('icon_key')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN icon_key TEXT');
  }
  if (!columns.contains('smoothing')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN smoothing INTEGER NOT NULL DEFAULT 0');
  }
  if (!columns.contains('line_style')) {
    await db.execute('ALTER TABLE $savedPathsTable ADD COLUMN line_style TEXT');
  }
}

Future<void> _migrateV1ToV2(Database db) async {
  await db.execute('''
    CREATE TABLE IF NOT EXISTS $pendingDeletesTable(
      uuid TEXT PRIMARY KEY,
      created_at TEXT NOT NULL
    )
  ''');
}

Future<void> _migrateV2ToV3(Database db) async {
  await db.execute('''
    CREATE TABLE IF NOT EXISTS $savedPathsTable(
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
  await db.execute('CREATE INDEX IF NOT EXISTS idx_saved_paths_bounds ON $savedPathsTable(min_lat, max_lat, min_lng, max_lng)');
}