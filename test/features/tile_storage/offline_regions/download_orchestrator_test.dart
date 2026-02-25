import 'dart:async';
import 'dart:io';
import 'dart:typed_data';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:path/path.dart' as p;
import 'package:shelf/shelf.dart' as shelf;
import 'package:shelf/shelf_io.dart' as shelf_io;
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/core/service/logger.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/region_repository.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/tile_job_queue.dart';
import 'package:turbo/features/tile_storage/tile_store/api.dart';
import 'package:turbo/features/tile_storage/tile_store/utils/tile_provider_id_sanitizer.dart';
import 'package:logging/logging.dart';

// A robust listener that allows tests to `await` the completion of a specific download.
final testDownloadCompletionProvider =
FutureProvider.family<void, String>((ref, regionId) {
  final completer = Completer<void>();
  ref.onDispose(() {
    if (!completer.isCompleted) {
      completer.completeError('Provider disposed before download completed.');
    }
  });

  final sub = ref.listen<AsyncValue<List<OfflineRegion>>>(
    offlineRegionsProvider,
        (prev, next) {
      if (next.hasValue) {
        OfflineRegion? region;
        try {
          // **THE FIX**: Use try-catch instead of a faulty orElse.
          // firstWhere throws StateError if no element is found.
          region = next.value!.firstWhere((r) => r.id == regionId);
        } on StateError {
          region = null; // This is the expected "not found" case.
        }

        if (region != null &&
            (region.status == DownloadStatus.completed || region.status == DownloadStatus.failed)) {
          if (!completer.isCompleted) {
            completer.complete();
          }
        }
      }
    },
    fireImmediately: true,
  );

  ref.onDispose(() => sub.close());
  return completer.future;
});

Future<ProviderContainer> createTestContainer(Directory tempDir) async {
  final dbPath = tempDir.path;
  final db = await databaseFactory.openDatabase(
    p.join(dbPath, 'turbo_app_v1.db'),
    options: OpenDatabaseOptions(version: 1, onCreate: _createDb),
  );

  return ProviderContainer(
    overrides: [
      databaseProvider.overrideWith((ref) async => db),
      tileStoreServiceProvider.overrideWith((ref) async {
        final testDb = await ref.watch(databaseProvider.future);
        return TileStoreService(testDb, testDirectory: tempDir.path);
      }),
    ],
  );
}

Future<void> _createDb(Database db, int version) async {
  final batch = db.batch();
  batch.execute('''
    CREATE TABLE offline_regions(
      id TEXT PRIMARY KEY, name TEXT NOT NULL, minLat REAL NOT NULL, minLng REAL NOT NULL,
      maxLat REAL NOT NULL, maxLng REAL NOT NULL, minZoom INTEGER NOT NULL, maxZoom INTEGER NOT NULL,
      urlTemplate TEXT NOT NULL, tileProviderId TEXT NOT NULL, tileProviderName TEXT NOT NULL,
      status INTEGER NOT NULL, totalTiles INTEGER NOT NULL, downloadedTiles INTEGER NOT NULL, createdAt TEXT NOT NULL
    )
  ''');
  batch.execute('''
    CREATE TABLE tile_jobs(
      regionId TEXT NOT NULL, providerId TEXT NOT NULL, z INTEGER NOT NULL, x INTEGER NOT NULL, y INTEGER NOT NULL,
      url TEXT NOT NULL, status INTEGER NOT NULL, attemptCount INTEGER NOT NULL DEFAULT 0,
      workerId TEXT, startedAt TEXT, PRIMARY KEY (regionId, z, x, y)
    )
  ''');
  batch.execute('CREATE INDEX idx_job_status ON tile_jobs (status)');
  batch.execute('''
    CREATE TABLE tile_store(
      providerId TEXT NOT NULL, z INTEGER NOT NULL, x INTEGER NOT NULL, y INTEGER NOT NULL,
      path TEXT NOT NULL, sizeInBytes INTEGER NOT NULL, lastAccessed TEXT NOT NULL,
      referenceCount INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (providerId, z, x, y)
    )
  ''');
  await batch.commit(noResult: true);
}


// A robust helper function to wait for the region to appear in the state.
Future<String> awaitRegionCreation(ProviderContainer container, String regionName) async {
  while (true) {
    final regions = await container.read(offlineRegionsProvider.future);
    OfflineRegion? region;
    try {
      // **THE FIX**: Use try-catch here as well.
      region = regions.firstWhere((r) => r.name == regionName);
    } on StateError {
      region = null;
    }
    if (region != null) {
      return region.id;
    }
    await Future.delayed(const Duration(milliseconds: 50));
  }
}

void main() {
  late ProviderContainer container;
  late OfflineApi offlineApi;
  late TileStoreService tileStoreService;
  late Directory tempDir;
  late Database db;
  HttpServer? server;
  late String urlTemplate;

  final tileBytes = Uint8List.fromList([1, 2, 3]);

  setUpAll(() {
    sqfliteFfiInit();
    databaseFactory = databaseFactoryFfi;
    setupLogging(level: Level.INFO);
  });

  setUp(() async {
    tempDir = await Directory.systemTemp.createTemp('offline_integration_test_');
    container = await createTestContainer(tempDir);
    db = await container.read(databaseProvider.future);

    await container.read(tileJobQueueProvider.future);
    tileStoreService = await container.read(tileStoreServiceProvider.future);
    await container.read(regionRepositoryProvider.future);
    await container.read(offlineRegionsProvider.future);
    container.read(downloadOrchestratorProvider);
    offlineApi = container.read(offlineApiProvider);
  });

  tearDown(() async {
    await server?.close(force: true);
    container.read(downloadOrchestratorProvider)?.stop();
    await Future.delayed(const Duration(milliseconds: 100));
    await db.close();
    container.dispose();
    if (await tempDir.exists()) {
      await tempDir.delete(recursive: true);
    }
  });

  Future<void> startServer(shelf.Handler handler) async {
    server = await shelf_io.serve(handler, InternetAddress.anyIPv4, 0);
    urlTemplate = 'http://127.0.0.1:${server!.port}/{z}/{x}/{y}.png';
  }

  group('Offline Download Integration Test with Real Isolates', () {
    test('successfully downloads a region', () async {
      const minZoom = 1;
      const maxZoom = 2;
      const expectedTileCount = 20;
      const regionName = 'Test Area Success';

      shelf.Response handler(shelf.Request request) {
        return shelf.Response.ok(tileBytes, headers: {'Content-Type': 'image/png'});
      }
      await startServer(handler);

      offlineApi.downloadRegion(
        name: regionName,
        bounds: LatLngBounds(const LatLng(-85, -180), const LatLng(85, 180)),
        minZoom: minZoom,
        maxZoom: maxZoom,
        urlTemplate: urlTemplate,
        tileProviderId: 'test_provider',
        tileProviderName: 'Test Provider',
      );

      final regionId = await awaitRegionCreation(container, regionName);
      await container.read(testDownloadCompletionProvider(regionId).future);

      final regions = await container.read(offlineRegionsProvider.future);
      final testRegion = regions.firstWhere((r) => r.name == regionName);

      expect(testRegion.status, DownloadStatus.completed);
      expect(testRegion.totalTiles, expectedTileCount);
      expect(testRegion.downloadedTiles, expectedTileCount);
    }, timeout: const Timeout(Duration(seconds: 30)));

    test('handles failed tiles gracefully', () async {
      const minZoom = 1;
      const maxZoom = 3;
      const totalTiles = 84;
      const failedTiles = 16;
      const expectedDownloadedCount = totalTiles - failedTiles;
      const regionName = 'Test Area With Failures';

      shelf.Response handler(shelf.Request request) {
        final path = request.url.path;
        if (path.startsWith('2/')) {
          return shelf.Response.internalServerError();
        }
        return shelf.Response.ok(tileBytes, headers: {'Content-Type': 'image/png'});
      }
      await startServer(handler);

      offlineApi.downloadRegion(
        name: regionName,
        bounds: LatLngBounds(const LatLng(-85, -180), const LatLng(85, 180)),
        minZoom: minZoom,
        maxZoom: maxZoom,
        urlTemplate: urlTemplate,
        tileProviderId: 'test_provider',
        tileProviderName: 'Test Provider',
      );

      final regionId = await awaitRegionCreation(container, regionName);
      await container.read(testDownloadCompletionProvider(regionId).future);

      final regions = await container.read(offlineRegionsProvider.future);
      final testRegion = regions.firstWhere((r) => r.name == regionName);

      expect(testRegion.status, DownloadStatus.completed);
      expect(testRegion.totalTiles, totalTiles);
      expect(testRegion.downloadedTiles, expectedDownloadedCount);

      final providerId = sanitizeProviderId(urlTemplate);
      final successTile = await tileStoreService.get(providerId, const TileCoordinates(0, 0, 1));
      expect(successTile, isNotNull);
      final failedTile = await tileStoreService.get(providerId, const TileCoordinates(0, 0, 2));
      expect(failedTile, isNull);
    }, timeout: const Timeout(Duration(seconds: 30)));
  });
}