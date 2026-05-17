import 'package:flutter_map/flutter_map.dart' show LatLngBounds;
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/features/tile_storage/offline_regions/data/region_repository.dart';
import 'package:turbo/features/tile_storage/offline_regions/models/offline_region.dart';

import '../../../helpers/in_memory_db.dart';
import '../../../helpers/pump_app.dart';

OfflineRegion _region(String id, DateTime createdAt) => OfflineRegion(
      id: id,
      name: 'r-$id',
      bounds: LatLngBounds(const LatLng(0, 0), const LatLng(1, 1)),
      minZoom: 0,
      maxZoom: 1,
      urlTemplate: 'https://example.com/{z}/{x}/{y}.png',
      tileProviderId: 'p',
      tileProviderName: 'P',
      createdAt: createdAt,
      totalTiles: 0,
      downloadedTiles: 0,
    );

void main() {
  late Database db;
  late RegionRepository repo;

  setUpAll(() {
    initSqfliteFfi();
  });

  setUp(() async {
    db = await databaseFactory.openDatabase(inMemoryDatabasePath);
    await db.execute('''
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
    repo = RegionRepository(db);
  });

  tearDown(() async {
    await db.close();
  });

  test('deleteOlderThan removes only stale regions and returns their ids',
      () async {
    final now = DateTime(2026, 5, 17);
    await repo.saveRegion(_region('old', now.subtract(const Duration(days: 200))));
    await repo.saveRegion(_region('mid', now.subtract(const Duration(days: 60))));
    await repo.saveRegion(_region('fresh', now.subtract(const Duration(days: 5))));

    final cutoff = now.subtract(const Duration(days: 90));
    final removed = await repo.deleteOlderThan(cutoff);

    expect(removed, ['old']);
    final remaining = await repo.getAllRegions();
    expect(remaining.map((r) => r.id).toSet(), {'mid', 'fresh'});
  });

  test('deleteOlderThan is a no-op when no region matches', () async {
    final now = DateTime(2026, 5, 17);
    await repo.saveRegion(_region('one', now.subtract(const Duration(days: 3))));

    final removed =
        await repo.deleteOlderThan(now.subtract(const Duration(days: 90)));
    expect(removed, isEmpty);
    expect((await repo.getAllRegions()).length, 1);
  });
}
