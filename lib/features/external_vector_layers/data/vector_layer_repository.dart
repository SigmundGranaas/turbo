import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/util/slippy_tiles.dart';
import '../models/vector_feature.dart';
import '../models/vector_layer_source.dart';
import 'sqlite_vector_tile_store.dart' show vectorTileStoreProvider;
import 'vector_layer_cache.dart';
import 'vector_layer_fetcher.dart';
import 'vector_tile_store.dart';

/// Wires the fetcher and tile store from their own providers. Falls back
/// to an in-memory-only setup if the persistent store isn't ready yet
/// (web, or during the brief window before `databaseProvider` resolves).
final vectorLayerRepositoryProvider = Provider<VectorLayerRepository>((ref) {
  final storeAsync = ref.watch(vectorTileStoreProvider);
  final store = storeAsync.asData?.value ?? NoopVectorTileStore();
  return VectorLayerRepository(
    fetcher: ref.watch(vectorLayerFetcherProvider),
    store: store,
  );
});

/// Two-tier cache for vector tiles. In-memory LRU on top of a
/// [VectorTileStore] that defaults to a no-op on web.
///
/// Responsibilities:
///   1. Dice a bbox query into a fixed-zoom slippy tile grid (the unit
///      of caching) — delegated to `core/util/slippy_tiles.dart`.
///   2. For each tile, return memory hits → persistent hits → network
///      fetch, in that order. Network failures degrade silently and
///      skip the offending tile.
///   3. Persist newly-fetched tiles via the supplied store, using each
///      feature's `toGeoJson()` for serialisation.
///
/// Feature dedupe across tile overlap is handled here so the consumer
/// sees a flat list with no repeats.
class VectorLayerRepository {
  final VectorLayerFetcher _fetcher;
  final VectorTileStore _store;
  final VectorLayerCache _memory;

  /// Vector tiles are valid for one week by default. Trail data
  /// changes rarely; sources can request a tighter window per fetch.
  static const Duration _defaultMaxAge = Duration(days: 7);

  /// Slippy-map zoom level the cache snaps queries to. z=12 returns
  /// reasonable feature counts across Norway.
  static const int gridZoom = 12;

  VectorLayerRepository({
    required VectorLayerFetcher fetcher,
    required VectorTileStore store,
    VectorLayerCache? memory,
  })  : _fetcher = fetcher,
        _store = store,
        _memory = memory ?? VectorLayerCache();

  /// Fetches features intersecting [bounds] from [source]. Hits memory
  /// cache first, then the persistent cache, then the network.
  Future<List<VectorFeature>> featuresInBounds(
    VectorLayerSource source,
    double minLat,
    double minLon,
    double maxLat,
    double maxLon, {
    Duration maxAge = _defaultMaxAge,
  }) async {
    final tiles = tilesCovering(
      zoom: gridZoom,
      minLat: minLat,
      minLon: minLon,
      maxLat: maxLat,
      maxLon: maxLon,
    );
    final seenFeatureIds = <String>{};
    final out = <VectorFeature>[];
    final now = DateTime.now();

    for (final tile in tiles) {
      final key = '${source.id}/${tile.z}/${tile.x}/${tile.y}';

      var features = _memory.get(key);
      features ??= await _readPersisted(source.id, tile, maxAge: maxAge);
      if (features != null) {
        _memory.put(key, features);
      } else {
        try {
          features = await _fetchTile(source, tile);
          _memory.put(key, features);
          if (source.persist) {
            await _writePersisted(source.id, tile, features, now);
          }
        } catch (_) {
          continue;
        }
      }

      for (final f in features) {
        if (seenFeatureIds.add(f.id)) out.add(f);
      }
    }
    return out;
  }

  Future<List<VectorFeature>> _fetchTile(
      VectorLayerSource source, SlippyTile tile) {
    final b = tile.bounds;
    return _fetcher.fetchBounds(
      source,
      minLat: b.south,
      minLon: b.west,
      maxLat: b.north,
      maxLon: b.east,
    );
  }

  Future<List<VectorFeature>?> _readPersisted(String source, SlippyTile tile,
      {required Duration maxAge}) async {
    final stored = await _store.read(source, tile.z, tile.x, tile.y);
    if (stored == null) return null;
    if (DateTime.now().difference(stored.fetchedAt) > maxAge) return null;
    if (stored.geojson.isEmpty) return const [];
    return VectorLayerFetcher.parseGeoJson(stored.geojson);
  }

  Future<void> _writePersisted(String source, SlippyTile tile,
      List<VectorFeature> features, DateTime fetchedAt) async {
    final fc = <String, Object?>{
      'type': 'FeatureCollection',
      'features': [for (final f in features) f.toGeoJson()],
    };
    await _store.write(
        source, tile.z, tile.x, tile.y, jsonEncode(fc), fetchedAt);
  }
}
