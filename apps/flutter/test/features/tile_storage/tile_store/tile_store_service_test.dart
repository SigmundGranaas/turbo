import 'dart:io';
import 'dart:typed_data';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/features/tile_storage/tile_store/data/tile_store_service.dart';

void main() {
  late TileStoreService tileStoreService;
  late Database db;
  late Directory tempDir;

  const providerId = 'test_provider';
  const coords1 = TileCoordinates(1, 1, 1);
  final tileBytes1 = Uint8List.fromList([1, 2, 3]);
  const coords2 = TileCoordinates(2, 2, 2);
  final tileBytes2 = Uint8List.fromList([4, 5, 6]);

  setUpAll(() {
    sqfliteFfiInit();
    databaseFactory = databaseFactoryFfi;
  });

  setUp(() async {
    tempDir = await Directory.systemTemp.createTemp('tile_store_test_');
    db = await databaseFactory.openDatabase(inMemoryDatabasePath);
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
    await db.execute('CREATE INDEX idx_tile_path ON tile_store (path)');
    await db.execute(
        'CREATE INDEX idx_tile_ref_count ON tile_store (referenceCount)');

    tileStoreService = TileStoreService(db, testDirectory: tempDir.path);
  });

  tearDown(() async {
    await db.close();
    if (await tempDir.exists()) {
      await tempDir.delete(recursive: true);
    }
  });

  group('TileStoreService Unit Tests', () {
    test('put and get should store and retrieve a tile from memory and disk',
            () async {
          expect(await tileStoreService.get(providerId, coords1), isNull);
          await tileStoreService.put(providerId, coords1, tileBytes1);
          final retrievedBytes = await tileStoreService.get(providerId, coords1);
          expect(retrievedBytes, equals(tileBytes1));

          final stats = await tileStoreService.getStats();
          expect(stats.memory.tileCount, 1);

          final tilePath = await tileStoreService.getTilePath(providerId, coords1);
          expect(await File(tilePath).exists(), isTrue);
          final diskStats = stats.disk;
          expect(diskStats.tileCount, 1);
          expect(diskStats.sizeInBytes, tileBytes1.length);
        });

    test('get should hit memory cache (L1) first', () async {
      await tileStoreService.put(providerId, coords1, tileBytes1);

      final tilePath = await tileStoreService.getTilePath(providerId, coords1);
      await File(tilePath).delete();

      final retrievedBytes = await tileStoreService.get(providerId, coords1);
      expect(retrievedBytes, equals(tileBytes1));
    });

    test('get should hit disk cache (L2) if not in memory', () async {
      await tileStoreService.put(providerId, coords1, tileBytes1);
      await tileStoreService.clearMemoryCache();

      final memoryStats = (await tileStoreService.getStats()).memory;
      expect(memoryStats.tileCount, 0);

      final retrievedBytes = await tileStoreService.get(providerId, coords1);
      expect(retrievedBytes, equals(tileBytes1));

      final newMemoryStats = (await tileStoreService.getStats()).memory;
      expect(newMemoryStats.tileCount, 1);
    });

    test('increment and decrement reference count should work', () async {
      await tileStoreService.put(providerId, coords1, tileBytes1);

      await tileStoreService.incrementReference(providerId, coords1);
      var record = await db
          .query('tile_store', where: 'providerId = ?', whereArgs: [providerId]);
      expect(record.first['referenceCount'], 1);

      await tileStoreService.incrementReference(providerId, coords1);
      record = await db
          .query('tile_store', where: 'providerId = ?', whereArgs: [providerId]);
      expect(record.first['referenceCount'], 2);

      await tileStoreService.decrementReference(providerId, coords1);
      record = await db
          .query('tile_store', where: 'providerId = ?', whereArgs: [providerId]);
      expect(record.first['referenceCount'], 1);
    });

    test('clearDiskCache should only remove unreferenced tiles', () async {
      await tileStoreService.put(providerId, coords1, tileBytes1);
      await tileStoreService.put(providerId, coords2, tileBytes2);
      await tileStoreService.incrementReference(providerId, coords2);

      var diskStats = await tileStoreService.getStats();
      expect(diskStats.disk.tileCount, 2);

      final clearedCount = await tileStoreService.clearDiskCache();
      expect(clearedCount, 1);

      // **THE FIX**: Clear memory cache to ensure we are reading from the disk state.
      await tileStoreService.clearMemoryCache();

      diskStats = await tileStoreService.getStats();
      expect(diskStats.disk.tileCount, 1);
      expect(await tileStoreService.get(providerId, coords1), isNull);
      expect(await tileStoreService.get(providerId, coords2), isNotNull);
    });
  });
}