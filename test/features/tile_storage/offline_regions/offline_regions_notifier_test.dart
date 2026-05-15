import 'dart:async';
import 'dart:io';

import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:path/path.dart' as p;
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/offline_tile_provider.dart';
import 'package:turbo/features/tile_storage/tile_store/api.dart';

Future<void> _createSchema(Database db, int version) async {
  final batch = db.batch();
  batch.execute('''
    CREATE TABLE offline_regions(
      id TEXT PRIMARY KEY, name TEXT NOT NULL, minLat REAL NOT NULL,
      minLng REAL NOT NULL, maxLat REAL NOT NULL, maxLng REAL NOT NULL,
      minZoom INTEGER NOT NULL, maxZoom INTEGER NOT NULL,
      urlTemplate TEXT NOT NULL, tileProviderId TEXT NOT NULL,
      tileProviderName TEXT NOT NULL, status INTEGER NOT NULL,
      totalTiles INTEGER NOT NULL, downloadedTiles INTEGER NOT NULL,
      createdAt TEXT NOT NULL
    )
  ''');
  batch.execute('''
    CREATE TABLE tile_jobs(
      regionId TEXT NOT NULL, providerId TEXT NOT NULL, z INTEGER NOT NULL,
      x INTEGER NOT NULL, y INTEGER NOT NULL, url TEXT NOT NULL,
      status INTEGER NOT NULL, attemptCount INTEGER NOT NULL DEFAULT 0,
      workerId TEXT, startedAt TEXT, PRIMARY KEY (regionId, z, x, y)
    )
  ''');
  batch.execute('''
    CREATE TABLE tile_store(
      providerId TEXT NOT NULL, z INTEGER NOT NULL, x INTEGER NOT NULL,
      y INTEGER NOT NULL, path TEXT NOT NULL, sizeInBytes INTEGER NOT NULL,
      lastAccessed TEXT NOT NULL, referenceCount INTEGER NOT NULL DEFAULT 0,
      PRIMARY KEY (providerId, z, x, y)
    )
  ''');
  await batch.commit(noResult: true);
}

OfflineRegion _region({
  String id = 'r1',
  String urlTemplate = 'https://tiles.example/{z}/{x}/{y}.png',
}) =>
    OfflineRegion(
      id: id,
      name: 'Test',
      bounds: LatLngBounds(const LatLng(59, 10), const LatLng(60, 11)),
      minZoom: 5,
      maxZoom: 7,
      urlTemplate: urlTemplate,
      tileProviderId: 'p',
      tileProviderName: 'P',
      status: DownloadStatus.completed,
      totalTiles: 0,
    );

void main() {
  late Directory tempDir;
  late Database db;

  setUpAll(() {
    sqfliteFfiInit();
    databaseFactory = databaseFactoryFfi;
  });

  setUp(() async {
    tempDir = await Directory.systemTemp.createTemp('offline_notifier_test_');
    db = await databaseFactory.openDatabase(
      p.join(tempDir.path, 'turbo_app_v1.db'),
      options: OpenDatabaseOptions(version: 1, onCreate: _createSchema),
    );
  });

  tearDown(() async {
    await db.close();
    if (await tempDir.exists()) {
      await tempDir.delete(recursive: true);
    }
  });

  group('OfflineRegionsNotifier.createTileProvider', () {
    test('returns an OfflineTileProvider when the tile store is ready',
        () async {
      final container = ProviderContainer(overrides: [
        databaseProvider.overrideWith((ref) async => db),
        tileStoreServiceProvider.overrideWith(
          (ref) async => TileStoreService(db, testDirectory: tempDir.path),
        ),
      ]);
      addTearDown(container.dispose);

      // Drive both async providers to their data states so the notifier sees
      // a real TileStoreService when it asks for one.
      await container.read(tileStoreServiceProvider.future);
      await container.read(offlineRegionsProvider.future);

      final notifier = container.read(offlineRegionsProvider.notifier);
      final provider = notifier.createTileProvider(region: _region());

      expect(provider, isA<OfflineTileProvider>());
    });

    test('returns null while the tile store is still loading', () async {
      // A future that never resolves keeps tileStoreServiceProvider in the
      // loading state for the duration of the test.
      final pending = Completer<TileStoreService>();
      addTearDown(() {
        if (!pending.isCompleted) {
          pending.complete(TileStoreService(db, testDirectory: tempDir.path));
        }
      });

      final container = ProviderContainer(overrides: [
        databaseProvider.overrideWith((ref) async => db),
        tileStoreServiceProvider.overrideWith((ref) => pending.future),
      ]);
      addTearDown(container.dispose);

      // Kick the provider so it transitions into AsyncLoading state.
      container.read(tileStoreServiceProvider);
      await container.read(offlineRegionsProvider.future);

      final notifier = container.read(offlineRegionsProvider.notifier);
      final provider = notifier.createTileProvider(region: _region());

      expect(provider, isNull);
    });
  });

  group('OfflineRegionsNotifier.createRegion', () {
    test('persists a new region and exposes it in the notifier state',
        () async {
      final container = ProviderContainer(overrides: [
        databaseProvider.overrideWith((ref) async => db),
      ]);
      addTearDown(container.dispose);

      // Initial state: no regions.
      final before = await container.read(offlineRegionsProvider.future);
      expect(before, isEmpty);

      await container.read(offlineRegionsProvider.notifier).createRegion(
            name: 'Oslo',
            bounds: LatLngBounds(
              const LatLng(59.8, 10.6),
              const LatLng(59.95, 10.85),
            ),
            minZoom: 8,
            maxZoom: 9,
            urlTemplate: 'https://tiles.example/{z}/{x}/{y}.png',
            tileProviderId: 'p',
            tileProviderName: 'P',
          );

      // The user-visible outcome: the new region shows up in the list with
      // its bounds and zoom range — and the totalTiles count reflects the
      // bounds × zoom computation now folded into createRegion.
      final after = container.read(offlineRegionsProvider).value!;
      expect(after, hasLength(1));
      final saved = after.single;
      expect(saved.name, 'Oslo');
      expect(saved.minZoom, 8);
      expect(saved.maxZoom, 9);
      expect(saved.totalTiles, greaterThan(0),
          reason: 'createRegion must compute coords from bounds');

      // And the region survives a fresh DB query (real persistence, not
      // just in-memory notifier state).
      final rows = await db.query('offline_regions');
      expect(rows, hasLength(1));
      expect(rows.single['name'], 'Oslo');
    });

    test('is a no-op when the bounds × zoom range yields zero tiles',
        () async {
      final container = ProviderContainer(overrides: [
        databaseProvider.overrideWith((ref) async => db),
      ]);
      addTearDown(container.dispose);

      await container.read(offlineRegionsProvider.future);

      // Degenerate bounds (zero area) at high zoom can still produce one tile
      // due to floor-rounding, so we use an inverted range — minZoom > maxZoom
      // — which makes the coord loop run zero iterations.
      await container.read(offlineRegionsProvider.notifier).createRegion(
            name: 'Empty',
            bounds: LatLngBounds(
              const LatLng(59.9, 10.7),
              const LatLng(59.91, 10.71),
            ),
            minZoom: 10,
            maxZoom: 9,
            urlTemplate: 'https://tiles.example/{z}/{x}/{y}.png',
            tileProviderId: 'p',
            tileProviderName: 'P',
          );

      // No region was created.
      final regions = container.read(offlineRegionsProvider).value!;
      expect(regions, isEmpty);
      final rows = await db.query('offline_regions');
      expect(rows, isEmpty);
    });
  });
}
