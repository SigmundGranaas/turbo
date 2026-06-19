import '../models/ntb_poi.dart';
import '../models/ntb_route.dart';
import 'ntb_client.dart';

/// Caching layer over [NtbClient]. Keeps viewport POI results keyed by a
/// quantised bbox (so small pans reuse a fetch) and fetched routes by id, both
/// in-memory with a short TTL — markers are not authoritative app data, just a
/// browsable overlay, so a process-lifetime memory cache is plenty.
class NtbRepository {
  final NtbClient client;
  final Duration ttl;
  final int maxBboxEntries;
  final int maxRouteEntries;

  NtbRepository({
    required this.client,
    this.ttl = const Duration(minutes: 15),
    this.maxBboxEntries = 32,
    this.maxRouteEntries = 64,
  });

  final Map<String, _CacheEntry<List<NtbPoi>>> _pois = {};
  final Map<String, _CacheEntry<NtbRoute?>> _routes = {};

  /// Quantise bounds to a ~0.05° grid so nearby viewports share a cache key.
  static String _bboxKey(
      double minLat, double minLon, double maxLat, double maxLon) {
    String q(double v) => (v / 0.05).round().toString();
    return '${q(minLat)}:${q(minLon)}:${q(maxLat)}:${q(maxLon)}';
  }

  Future<List<NtbPoi>> poisInBounds({
    required double minLat,
    required double minLon,
    required double maxLat,
    required double maxLon,
  }) async {
    final key = _bboxKey(minLat, minLon, maxLat, maxLon);
    final cached = _pois[key];
    if (cached != null && !cached.isStale(ttl)) return cached.value;

    final pois = await client.fetchPois(
      minLat: minLat,
      minLon: minLon,
      maxLat: maxLat,
      maxLon: maxLon,
    );
    _put(_pois, key, pois, maxBboxEntries);
    return pois;
  }

  Future<NtbRoute?> route(String turId) async {
    final cached = _routes[turId];
    if (cached != null && !cached.isStale(ttl)) return cached.value;
    final route = await client.fetchRoute(turId);
    _put(_routes, turId, route, maxRouteEntries);
    return route;
  }

  static void _put<T>(
      Map<String, _CacheEntry<T>> map, String key, T value, int maxEntries) {
    map[key] = _CacheEntry(value);
    if (map.length > maxEntries) {
      // Evict the oldest insertion (Dart maps preserve insertion order).
      map.remove(map.keys.first);
    }
  }
}

class _CacheEntry<T> {
  final T value;
  final DateTime at;
  _CacheEntry(this.value) : at = DateTime.now();
  bool isStale(Duration ttl) => DateTime.now().difference(at) > ttl;
}
