import 'dart:convert';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/data/database_provider.dart';
import '../models/vector_feature.dart';
import '../models/vector_layer_source.dart';
import 'sqlite_vector_tile_store.dart';
import 'vector_layer_cache.dart';
import 'vector_layer_fetcher.dart';
import 'vector_tile_store.dart';

/// Two-tier cache for vector tiles. In-memory LRU on top of a [VectorTileStore]
/// that defaults to a no-op on web.
class VectorLayerRepository {
  final VectorLayerFetcher _fetcher;
  final VectorTileStore _store;
  final VectorLayerCache _memory;

  /// Vector tiles are valid for one week by default. Trail data changes
  /// rarely; sources can request a tighter window per fetch.
  static const Duration _defaultMaxAge = Duration(days: 7);

  /// Slippy-map zoom level used to derive the tile grid we snap requests
  /// to. z=12 returns reasonable feature counts across Norway.
  static const int gridZoom = 12;

  VectorLayerRepository({
    required VectorLayerFetcher fetcher,
    required VectorTileStore store,
    VectorLayerCache? memory,
  })  : _fetcher = fetcher,
        _store = store,
        _memory = memory ?? VectorLayerCache();

  /// Fetches features intersecting [bounds] from [source]. Hits memory cache
  /// first, then the persistent cache, then the network. Network failures
  /// degrade silently and skip the offending tile.
  Future<List<VectorFeature>> featuresInBounds(
    VectorLayerSource source,
    double minLat,
    double minLon,
    double maxLat,
    double maxLon, {
    Duration maxAge = _defaultMaxAge,
  }) async {
    final tiles = _tilesCovering(minLat, minLon, maxLat, maxLon);
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
      VectorLayerSource source, _TileCoords tile) {
    final b = tile.bounds;
    return _fetcher.fetchBounds(
      source,
      minLat: b.south,
      minLon: b.west,
      maxLat: b.north,
      maxLon: b.east,
    );
  }

  Future<List<VectorFeature>?> _readPersisted(String source, _TileCoords tile,
      {required Duration maxAge}) async {
    final stored = await _store.read(source, tile.z, tile.x, tile.y);
    if (stored == null) return null;
    if (DateTime.now().difference(stored.fetchedAt) > maxAge) return null;
    if (stored.geojson.isEmpty) return const [];
    return VectorLayerFetcher.parseGeoJson(stored.geojson);
  }

  Future<void> _writePersisted(String source, _TileCoords tile,
      List<VectorFeature> features, DateTime fetchedAt) async {
    final fc = <String, Object?>{
      'type': 'FeatureCollection',
      'features': [
        for (final f in features)
          {
            'type': 'Feature',
            'id': f.id,
            'properties': f.properties,
            'geometry': _geometryFor(f),
          },
      ],
    };
    await _store.write(
        source, tile.z, tile.x, tile.y, jsonEncode(fc), fetchedAt);
  }

  Map<String, Object?> _geometryFor(VectorFeature f) {
    List<List<List<double>>> ringsAsCoords() => [
          for (final ring in f.rings)
            [
              for (final p in ring) [p.longitude, p.latitude]
            ]
        ];
    if (f.kind == VectorGeometryKind.line) {
      if (f.rings.length == 1) {
        return {
          'type': 'LineString',
          'coordinates': [
            for (final p in f.rings.first) [p.longitude, p.latitude]
          ],
        };
      }
      return {
        'type': 'MultiLineString',
        'coordinates': ringsAsCoords(),
      };
    }
    if (f.rings.length == 1) {
      return {
        'type': 'Polygon',
        'coordinates': [
          [for (final p in f.rings.first) [p.longitude, p.latitude]]
        ],
      };
    }
    return {
      'type': 'MultiPolygon',
      'coordinates': [
        for (final ring in f.rings)
          [
            [for (final p in ring) [p.longitude, p.latitude]]
          ],
      ],
    };
  }

  List<_TileCoords> _tilesCovering(
      double minLat, double minLon, double maxLat, double maxLon) {
    final tl = _lonLatToTile(minLon, maxLat, gridZoom);
    final br = _lonLatToTile(maxLon, minLat, gridZoom);
    final tiles = <_TileCoords>[];
    final maxIndex = (1 << gridZoom) - 1;
    final x0 = math.min(tl.x, br.x);
    final x1 = math.max(tl.x, br.x);
    final y0 = math.min(tl.y, br.y);
    final y1 = math.max(tl.y, br.y);
    for (var x = x0; x <= x1; x++) {
      for (var y = y0; y <= y1; y++) {
        if (x < 0 || y < 0 || x > maxIndex || y > maxIndex) continue;
        tiles.add(_TileCoords(gridZoom, x, y));
      }
    }
    return tiles;
  }

  static _TileCoords _lonLatToTile(double lon, double lat, int z) {
    final n = 1 << z;
    final x = ((lon + 180.0) / 360.0 * n).floor();
    final latRad = lat * math.pi / 180.0;
    final y = ((1.0 -
                (math.log(math.tan(latRad) + 1 / math.cos(latRad)) / math.pi)) /
            2.0 *
            n)
        .floor();
    return _TileCoords(z, x.clamp(0, n - 1), y.clamp(0, n - 1));
  }
}

class _TileCoords {
  final int z;
  final int x;
  final int y;
  const _TileCoords(this.z, this.x, this.y);

  _TileBounds get bounds {
    final n = 1 << z;
    final lonMin = x / n * 360.0 - 180.0;
    final lonMax = (x + 1) / n * 360.0 - 180.0;
    final latMax = _tileLat(y, n);
    final latMin = _tileLat(y + 1, n);
    return _TileBounds(
        north: latMax, south: latMin, east: lonMax, west: lonMin);
  }

  static double _tileLat(int y, int n) {
    final s = math.pi - 2.0 * math.pi * y / n;
    return 180.0 / math.pi * math.atan(0.5 * (math.exp(s) - math.exp(-s)));
  }
}

class _TileBounds {
  final double north;
  final double south;
  final double east;
  final double west;
  const _TileBounds({
    required this.north,
    required this.south,
    required this.east,
    required this.west,
  });
}

// === Public Riverpod providers ===========================================

/// On mobile/desktop builds, the app overrides this with a SQLite-backed
/// store after the `databaseProvider` resolves. On web (and in tests) we
/// fall back to a no-op store and the in-memory cache.
final vectorTileStoreProvider = FutureProvider<VectorTileStore>((ref) async {
  if (kIsWeb) return NoopVectorTileStore();
  final db = await ref.watch(databaseProvider.future);
  return SqliteVectorTileStore(db);
});

final vectorLayerFetcherProvider =
    Provider<VectorLayerFetcher>((_) => VectorLayerFetcher());

final vectorLayerRepositoryProvider = Provider<VectorLayerRepository>((ref) {
  final storeAsync = ref.watch(vectorTileStoreProvider);
  final store = storeAsync.asData?.value ?? NoopVectorTileStore();
  return VectorLayerRepository(
    fetcher: ref.watch(vectorLayerFetcherProvider),
    store: store,
  );
});
