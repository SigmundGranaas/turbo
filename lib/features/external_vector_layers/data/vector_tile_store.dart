/// One persisted vector-tile row: the JSON-encoded `FeatureCollection`
/// payload that the source returned for this tile, plus when we fetched it.
class StoredVectorTile {
  final String geojson;
  final DateTime fetchedAt;
  const StoredVectorTile({required this.geojson, required this.fetchedAt});
}

/// Persistent backing store for vector tile payloads. Implementations exist
/// for SQLite (mobile + desktop) and a no-op (web — in-memory cache only).
abstract class VectorTileStore {
  Future<StoredVectorTile?> read(String source, int z, int x, int y);
  Future<void> write(
      String source, int z, int x, int y, String geojson, DateTime fetchedAt);
}

class NoopVectorTileStore implements VectorTileStore {
  @override
  Future<StoredVectorTile?> read(String source, int z, int x, int y) async =>
      null;

  @override
  Future<void> write(String source, int z, int x, int y, String geojson,
      DateTime fetchedAt) async {}
}
