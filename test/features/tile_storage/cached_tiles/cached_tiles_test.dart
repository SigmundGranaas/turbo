import 'dart:io';
import 'dart:typed_data';
import 'package:dio/dio.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http_mock_adapter/http_mock_adapter.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/features/tile_storage/cached_tiles/data/cache_service.dart';
import 'package:turbo/features/tile_storage/tile_store/api.dart';

// Helper to create a test container with a mocked database and other dependencies.
Future<ProviderContainer> createTestContainer(
    Directory tempDir, Dio dio) async {
  final db = await databaseFactory.openDatabase(inMemoryDatabasePath);
  // Create tables needed by the services under test.
  await db.execute('''
    CREATE TABLE tile_store(
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

  return ProviderContainer(
    overrides: [
      databaseProvider.overrideWith((ref) async => db),
      dioProvider.overrideWithValue(dio),
      // This override is now simpler as it can rely on the overridden databaseProvider.
      tileStoreServiceProvider.overrideWith((ref) async {
        final testDb = await ref.watch(databaseProvider.future);
        return TileStoreService(testDb, testDirectory: tempDir.path);
      }),
    ],
  );
}

void main() {
  late ProviderContainer container;
  late CacheService cacheService;
  late TileStoreService tileStoreService;
  late Dio dio;
  late DioAdapter dioAdapter;
  late Directory tempDir;
  late Database db;

  const urlTemplate = 'https://tile.example.com/{z}/{x}/{y}.png';
  final tileUrl = urlTemplate.replaceAll('{z}', '1').replaceAll('{x}', '2').replaceAll('{y}', '3');
  final providerId = sanitizeProviderId(urlTemplate);
  const coords = TileCoordinates(2, 3, 1);
  final tileBytes = Uint8List.fromList([1, 2, 3, 4, 5]);

  setUpAll(() {
    TestWidgetsFlutterBinding.ensureInitialized();
    sqfliteFfiInit();
    databaseFactory = databaseFactoryFfi;
  });

  setUp(() async {
    tempDir = await Directory.systemTemp.createTemp('cache_api_test_');
    dio = Dio();
    dioAdapter = DioAdapter(dio: dio);
    container = await createTestContainer(tempDir, dio);

    // Asynchronously resolve the providers before running tests.
    cacheService = await container.read(cacheServiceProvider.future);
    tileStoreService = await container.read(tileStoreServiceProvider.future);
    db = await container.read(databaseProvider.future);
  });

  tearDown(() async {
    await db.close();
    container.dispose();
    if (await tempDir.exists()) {
      await tempDir.delete(recursive: true);
    }
  });

  group('CacheService Logic Integration Test', () {
    test('on CACHE HIT: should return bytes from store and NOT call network', () async {
      await tileStoreService.put(providerId, coords, tileBytes);
      dioAdapter.onGet(tileUrl, (server) => server.throws(500, DioException(requestOptions: RequestOptions())));

      final bytes = await cacheService.getTileBytes(providerId, coords, tileUrl, null);

      expect(bytes, isNotNull);
      expect(bytes, equals(tileBytes));
    });

    test('on CACHE MISS: should fetch from network and write to store', () async {
      expect(await tileStoreService.get(providerId, coords), isNull);
      dioAdapter.onGet(tileUrl, (server) => server.reply(200, tileBytes));

      final bytes = await cacheService.getTileBytes(providerId, coords, tileUrl, null);

      expect(bytes, isNotNull);
      expect(bytes, equals(tileBytes));

      await Future.delayed(const Duration(milliseconds: 100));
      final storedBytes = await tileStoreService.get(providerId, coords);
      expect(storedBytes, isNotNull);
      expect(storedBytes, equals(tileBytes));
    });

    test('clearCache should remove unreferenced tiles from the TileStore', () async {
      await tileStoreService.put(providerId, coords, tileBytes);
      expect(await tileStoreService.get(providerId, coords), isNotNull);

      final cacheService = await container.read(cacheServiceProvider.future);
      await cacheService.clear();

      expect(await tileStoreService.get(providerId, coords), isNull);
    });

    test(
        'WMS providers cache by (providerId, coords) independent of the '
        'per-tile URL — repeated fetches at the same tile coordinates with '
        'differing bbox-substituted URLs share the same cache entry',
        () async {
      const wmsTemplate =
          'https://example.com/wms?LAYERS=topo&FORMAT=image/png&'
          'SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&SRS=EPSG:3857&'
          'BBOX={bbox}&WIDTH={width}&HEIGHT={height}';
      final wmsProviderId = sanitizeProviderId(wmsTemplate);
      const wmsCoords = TileCoordinates(5, 7, 9);
      final wmsBytes = Uint8List.fromList([10, 20, 30]);

      // First fetch: store empty -> network supplies bytes, store gets
      // populated.
      const firstUrl =
          'https://example.com/wms?LAYERS=topo&FORMAT=image/png&'
          'SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&SRS=EPSG:3857&'
          'BBOX=1,2,3,4&WIDTH=256&HEIGHT=256';
      dioAdapter.onGet(firstUrl, (server) => server.reply(200, wmsBytes));

      final first = await cacheService.getTileBytes(
          wmsProviderId, wmsCoords, firstUrl, null);
      expect(first, wmsBytes);

      // Let the unawaited store.put finish before the second fetch.
      await Future<void>.delayed(const Duration(milliseconds: 50));

      // Second fetch: URL string is slightly different (different bbox
      // precision — common when the same tile is re-requested after a
      // zoom-pan cycle), but providerId + coords match. The cache should
      // serve from store WITHOUT touching the network.
      const secondUrl =
          'https://example.com/wms?LAYERS=topo&FORMAT=image/png&'
          'SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&SRS=EPSG:3857&'
          'BBOX=1.00000001,2.00000001,3,4&WIDTH=256&HEIGHT=256';
      // No dio handler registered for secondUrl — if the cache misses,
      // this errors out.
      final second = await cacheService.getTileBytes(
          wmsProviderId, wmsCoords, secondUrl, null);
      expect(second, wmsBytes,
          reason:
              'cache must serve by (providerId, coords); URL bytes-equality is not required');
    });
  });
}