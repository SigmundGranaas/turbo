import 'package:sqflite/sqflite.dart';

import 'package:turbo/core/data/database_provider.dart';
import 'vector_tile_store.dart';

/// SQLite-backed [VectorTileStore]. One row per (source, z, x, y).
class SqliteVectorTileStore implements VectorTileStore {
  final Database db;
  SqliteVectorTileStore(this.db);

  @override
  Future<StoredVectorTile?> read(String source, int z, int x, int y) async {
    final rows = await db.query(
      vectorTileCacheTable,
      where: 'source = ? AND z = ? AND x = ? AND y = ?',
      whereArgs: [source, z, x, y],
      limit: 1,
    );
    if (rows.isEmpty) return null;
    final row = rows.first;
    final ms = (row['fetched_at'] as num?)?.toInt() ?? 0;
    return StoredVectorTile(
      geojson: row['geojson'] as String? ?? '',
      fetchedAt: DateTime.fromMillisecondsSinceEpoch(ms),
    );
  }

  @override
  Future<void> write(String source, int z, int x, int y, String geojson,
      DateTime fetchedAt) async {
    await db.insert(
      vectorTileCacheTable,
      {
        'source': source,
        'z': z,
        'x': x,
        'y': y,
        'geojson': geojson,
        'fetched_at': fetchedAt.millisecondsSinceEpoch,
      },
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }
}
