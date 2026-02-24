import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:sqflite/sqflite.dart';
import 'package:turbo/features/tile_storage/offline_regions/models/offline_region.dart';

import '../../../../core/data/database_provider.dart';

final regionRepositoryProvider = FutureProvider<RegionRepository>((ref) async {
  // This will throw if run on web, which is correct as this feature is not for web.
  final db = await ref.watch(databaseProvider.future);
  return RegionRepository(db);
});

/// Manages the metadata for downloaded offline map regions.
class RegionRepository {
  final Database db;

  RegionRepository(this.db);

  Future<void> saveRegion(OfflineRegion region) async {
    await db.insert(
      regionsTable,
      region.toMap(),
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  /// Atomically increments the downloaded tile count for a region.
  Future<void> incrementDownloadedTileCount(String regionId, {int count = 1}) async {
    await db.rawUpdate(
        'UPDATE $regionsTable SET downloadedTiles = downloadedTiles + ? WHERE id = ?',
        [count, regionId]);
  }

  Future<OfflineRegion?> getRegion(String id) async {
    final maps =
    await db.query(regionsTable, where: 'id = ?', whereArgs: [id], limit: 1);
    return maps.isNotEmpty ? OfflineRegion.fromMap(maps.first) : null;
  }

  Future<List<OfflineRegion>> getAllRegions() async {
    final maps = await db.query(regionsTable, orderBy: 'createdAt DESC');
    return maps.map((map) => OfflineRegion.fromMap(map)).toList();
  }

  Future<void> deleteRegion(String id) async {
    await db.delete(regionsTable, where: 'id = ?', whereArgs: [id]);
  }
}