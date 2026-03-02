import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/features/saved_paths/data/path_search_service.dart';
import 'package:turbo/features/saved_paths/data/saved_path_repository.dart';
import 'package:turbo/features/saved_paths/data/sqlite_saved_path_datastore.dart';
import 'package:turbo/features/saved_paths/models/saved_path.dart';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

SavedPath _makePath({
  String? uuid,
  String title = 'Test Path',
  String? description,
  List<LatLng>? points,
  double distance = 1000.0,
  String? colorHex,
  String? iconKey,
  bool smoothing = false,
  String? lineStyleKey,
}) =>
    SavedPath(
      uuid: uuid,
      title: title,
      description: description,
      points: points ?? [const LatLng(59.9, 10.7), const LatLng(60.0, 10.8)],
      distance: distance,
      colorHex: colorHex,
      iconKey: iconKey,
      smoothing: smoothing,
      lineStyleKey: lineStyleKey,
    );

Future<List<SavedPath>> _waitForData(ProviderContainer container) async {
  for (var i = 0; i < 100; i++) {
    await Future.delayed(const Duration(milliseconds: 20));
    final s = container.read(savedPathRepositoryProvider);
    if (s is AsyncData<List<SavedPath>>) return s.value;
    if (s is AsyncError) throw (s as AsyncError).error;
  }
  throw TimeoutException('SavedPathRepository did not settle');
}

Future<Database> _createTestDb() async {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

void main() {
  late Database db;
  late ProviderContainer container;

  setUpAll(() {
    sqfliteFfiInit();
    databaseFactory = databaseFactoryFfi;
  });

  setUp(() async {
    db = await _createTestDb();
    container = ProviderContainer(overrides: [
      databaseProvider.overrideWith((ref) async => db),
    ]);
    container.listen(savedPathRepositoryProvider, (_, _) {});
    await _waitForData(container);
  });

  tearDown(() async {
    container.dispose();
    await db.close();
  });

  // Full user journey: save a path, rename it, verify in DB, then delete it.
  test('create → update → delete lifecycle persists through DB', () async {
    final repo = container.read(savedPathRepositoryProvider.notifier);

    // Save a path (user finishes measuring tool and taps "Save")
    final path = _makePath(
      uuid: 'hike-1',
      title: 'Besseggen Ridge',
      description: 'Summer hike',
      points: [
        const LatLng(61.50, 8.75),
        const LatLng(61.51, 8.77),
        const LatLng(61.52, 8.80),
      ],
      distance: 4200.0,
    );
    await repo.addPath(path);
    var paths = await _waitForData(container);
    expect(paths.length, 1);
    expect(paths.first.title, 'Besseggen Ridge');

    // Verify it actually hit the database, not just in-memory state
    final store = SQLiteSavedPathDataStore(db);
    final fromDb = await store.getByUuid('hike-1');
    expect(fromDb!.points.length, 3);
    expect(fromDb.distance, 4200.0);
    expect(fromDb.description, 'Summer hike');

    // User taps the path on the map, edits the name
    await repo.updatePath(path.copyWith(title: 'Besseggen via Memurubu'));
    paths = await _waitForData(container);
    expect(paths.first.title, 'Besseggen via Memurubu');

    // User deletes the path
    await repo.deletePath('hike-1');
    paths = await _waitForData(container);
    expect(paths, isEmpty);
    expect(await store.getByUuid('hike-1'), isNull);
  });

  // Spatial queries are the hardest logic — bounding box overlap with
  // inside/outside/partial-overlap cases.
  test('findInBounds returns overlapping paths, excludes distant ones', () async {
    final store = SQLiteSavedPathDataStore(db);

    // Path in Oslo area (fully inside viewport)
    await store.insert(_makePath(
      uuid: 'oslo',
      title: 'Oslo Trail',
      points: [const LatLng(59.9, 10.7), const LatLng(59.95, 10.8)],
    ));
    // Path in Bergen (fully outside viewport)
    await store.insert(_makePath(
      uuid: 'bergen',
      title: 'Bergen Trail',
      points: [const LatLng(60.39, 5.32), const LatLng(60.40, 5.33)],
    ));
    // Path stretching from south of viewport into it (partial overlap)
    await store.insert(_makePath(
      uuid: 'partial',
      title: 'Coastal Path',
      points: [const LatLng(58.0, 10.0), const LatLng(59.95, 11.0)],
    ));

    // Viewport centered on Oslo
    final results = await store.findInBounds(
      const LatLng(59.5, 10.0), // SW
      const LatLng(60.2, 11.5), // NE
    );

    final titles = results.map((p) => p.title).toSet();
    expect(titles, contains('Oslo Trail'));
    expect(titles, contains('Coastal Path'));
    expect(titles, isNot(contains('Bergen Trail')));
  });

  // Search integration: user types in search bar, finds saved paths by name.
  test('search finds paths by partial name and returns correct position', () async {
    final store = SQLiteSavedPathDataStore(db);
    await store.insert(_makePath(
      title: 'Trolltunga Hike',
      points: [const LatLng(60.12, 6.74), const LatLng(60.13, 6.75)],
    ));
    await store.insert(_makePath(title: 'Preikestolen Trail'));
    await store.insert(_makePath(title: 'Kjeragbolten Route'));

    final service = container.read(pathSearchServiceProvider);

    // Partial match, case-insensitive
    final results = await service.findLocationsBy('troll');
    expect(results.length, 1);
    expect(results.first.title, 'Trolltunga Hike');
    expect(results.first.source, 'saved_path');
    // Position should be the path's first point
    expect(results.first.position.latitude, closeTo(60.12, 0.01));

    // No match
    expect(await service.findLocationsBy('galdhøpiggen'), isEmpty);
  });

  test('customization fields round-trip through DB', () async {
    final repo = container.read(savedPathRepositoryProvider.notifier);

    final path = _makePath(
      uuid: 'custom-1',
      title: 'Custom Path',
      colorHex: '1976D2',
      iconKey: 'Vandring',
      smoothing: true,
      lineStyleKey: 'dashed',
    );
    await repo.addPath(path);
    var paths = await _waitForData(container);
    expect(paths.length, 1);

    final store = SQLiteSavedPathDataStore(db);
    final fromDb = await store.getByUuid('custom-1');
    expect(fromDb!.colorHex, '1976D2');
    expect(fromDb.iconKey, 'Vandring');
    expect(fromDb.smoothing, true);
    expect(fromDb.lineStyleKey, 'dashed');
  });

  test('v3→v4 migration preserves existing data, new fields default correctly', () async {
    // Simulate a v3 database
    final migrationDb = await databaseFactory.openDatabase(
      inMemoryDatabasePath,
      options: OpenDatabaseOptions(singleInstance: false),
    );
    await migrationDb.execute('''
      CREATE TABLE $savedPathsTable(
        uuid TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT,
        points TEXT NOT NULL, distance REAL NOT NULL,
        min_lat REAL NOT NULL, min_lng REAL NOT NULL,
        max_lat REAL NOT NULL, max_lng REAL NOT NULL,
        created_at TEXT NOT NULL
      )
    ''');
    // Insert a path with v3 schema
    await migrationDb.insert(savedPathsTable, {
      'uuid': 'old-path',
      'title': 'Old Path',
      'points': '[[59.9,10.7],[60.0,10.8]]',
      'distance': 1000.0,
      'min_lat': 59.9,
      'min_lng': 10.7,
      'max_lat': 60.0,
      'max_lng': 10.8,
      'created_at': DateTime.now().toIso8601String(),
    });

    // Apply v3→v4 migration
    await migrationDb.execute('ALTER TABLE $savedPathsTable ADD COLUMN color_hex TEXT');
    await migrationDb.execute('ALTER TABLE $savedPathsTable ADD COLUMN icon_key TEXT');
    await migrationDb.execute('ALTER TABLE $savedPathsTable ADD COLUMN smoothing INTEGER NOT NULL DEFAULT 0');
    await migrationDb.execute('ALTER TABLE $savedPathsTable ADD COLUMN line_style TEXT');

    // Verify old data survived
    final rows = await migrationDb.query(savedPathsTable, where: 'uuid = ?', whereArgs: ['old-path']);
    expect(rows.length, 1);
    expect(rows.first['title'], 'Old Path');
    expect(rows.first['color_hex'], isNull);
    expect(rows.first['icon_key'], isNull);
    expect(rows.first['smoothing'], 0);
    expect(rows.first['line_style'], isNull);

    await migrationDb.close();
  });

  // Migration safety: existing v2 marker data survives the v3 migration.
  test('v2→v3 migration preserves existing markers and adds saved_paths', () async {
    // Use a separate in-memory database so it doesn't conflict with setUp's db.
    final migrationDb = await databaseFactory.openDatabase(
      inMemoryDatabasePath,
      options: OpenDatabaseOptions(singleInstance: false),
    );
    await migrationDb.execute('''
      CREATE TABLE $markersTable(
        uuid TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT,
        icon TEXT, latitude REAL NOT NULL, longitude REAL NOT NULL,
        synced INTEGER NOT NULL DEFAULT 0
      )
    ''');
    await migrationDb.insert(markersTable, {
      'uuid': 'old-marker',
      'title': 'My Cabin',
      'latitude': 61.0,
      'longitude': 9.0,
      'synced': 1,
    });

    // Apply the same migration as _migrateV2ToV3
    await migrationDb.execute('''
      CREATE TABLE $savedPathsTable(
        uuid TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT,
        points TEXT NOT NULL, distance REAL NOT NULL,
        min_lat REAL NOT NULL, min_lng REAL NOT NULL,
        max_lat REAL NOT NULL, max_lng REAL NOT NULL,
        created_at TEXT NOT NULL,
        color_hex TEXT, icon_key TEXT,
        smoothing INTEGER NOT NULL DEFAULT 0,
        line_style TEXT
      )
    ''');
    await migrationDb.execute(
        'CREATE INDEX idx_saved_paths_bounds ON $savedPathsTable(min_lat, max_lat, min_lng, max_lng)');

    // Old data survived
    final markers = await migrationDb.query(markersTable);
    expect(markers.first['title'], 'My Cabin');

    // New table works
    final store = SQLiteSavedPathDataStore(migrationDb);
    await store.insert(_makePath(title: 'New Path'));
    expect((await store.getAll()).length, 1);

    await migrationDb.close();
  });
}
