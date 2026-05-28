import 'dart:async';

import 'package:flutter_map/flutter_map.dart' show LatLngBounds;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import '../models/weather_forecast.dart';
import 'weather_notifier.dart' show yrOceanServiceProvider;

/// A single marine reading positioned at a sampled grid point on the map.
class OceanGridSample {
  final LatLng position;
  final MarinePoint point;

  const OceanGridSample({required this.position, required this.point});
}

/// In-memory cache entry for one grid coordinate. [point] is `null` when MET
/// confirmed the coordinate has no marine coverage (land / outside the Nordic
/// footprint) — we cache those too so panning over a coastline doesn't refetch
/// the same dry points on every move.
class _CachedSample {
  final MarinePoint? point;
  final DateTime expiresAt;

  const _CachedSample(this.point, this.expiresAt);
}

/// Samples MET Norway's `oceanforecast/2.0` endpoint across the visible
/// viewport to drive the ocean-conditions map overlay.
///
/// The endpoint is a per-coordinate point query, so we lay a coarse grid over
/// the current bounds and fetch each cell. Calls are debounced (to coalesce a
/// pan gesture), capped in concurrency, and cached by rounded coordinate +
/// MET's `Expires` so repeated viewing of the same area stays cheap and within
/// MET's fair-use terms.
final oceanConditionsProvider =
    AsyncNotifierProvider<OceanConditionsNotifier, List<OceanGridSample>>(
  OceanConditionsNotifier.new,
);

class OceanConditionsNotifier extends AsyncNotifier<List<OceanGridSample>> {
  /// Number of cells per axis — a 5×5 grid yields up to 25 sampled points,
  /// trimmed by the cache and land/no-coverage responses.
  static const int _cellsPerAxis = 5;

  /// Maximum simultaneous in-flight requests to MET.
  static const int _maxConcurrent = 6;

  /// Fallback freshness when MET omits an `Expires` header.
  static const Duration _fallbackTtl = Duration(minutes: 30);

  Timer? _debounce;
  LatLngBounds? _pending;
  int _requestSeq = 0;
  final Map<String, _CachedSample> _cache = {};

  @override
  Future<List<OceanGridSample>> build() async {
    ref.onDispose(() => _debounce?.cancel());
    return const [];
  }

  /// Debounced refresh: schedules a sampling pass ~500ms after the last
  /// invocation so a panning gesture collapses into a single batch.
  void requestBounds(LatLngBounds bounds) {
    _pending = bounds;
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 500), _execute);
  }

  /// Clears the rendered samples (e.g. when the layer is toggled off). The
  /// coordinate cache is intentionally retained so re-enabling is instant.
  void clear() {
    _debounce?.cancel();
    _pending = null;
    state = const AsyncValue.data([]);
  }

  Future<void> _execute() async {
    final bounds = _pending;
    if (bounds == null) return;

    final seq = ++_requestSeq;
    final service = ref.read(yrOceanServiceProvider);
    final now = DateTime.now().toUtc();
    final queue = _buildGrid(bounds);
    final results = <OceanGridSample>[];

    Future<void> worker() async {
      while (queue.isNotEmpty) {
        if (seq != _requestSeq) return;
        final pos = queue.removeLast();
        final key = _key(pos);

        MarinePoint? point;
        final cached = _cache[key];
        if (cached != null && cached.expiresAt.isAfter(now)) {
          point = cached.point;
        } else {
          try {
            final res = await service.fetch(pos);
            point = (res != null && res.points.isNotEmpty)
                ? res.points.first
                : null;
            _cache[key] = _CachedSample(
              point,
              res?.expiresAt ?? now.add(_fallbackTtl),
            );
          } catch (_) {
            // Transient failure for one cell — skip it rather than failing
            // the whole layer. It'll be retried on the next refresh.
            continue;
          }
        }

        if (point != null && point.waveHeightM != null) {
          results.add(OceanGridSample(position: pos, point: point));
        }
      }
    }

    await Future.wait([for (var i = 0; i < _maxConcurrent; i++) worker()]);

    // A newer request superseded this one while we were fetching — drop the
    // stale results so the layer reflects the latest viewport only.
    if (seq != _requestSeq) return;
    state = AsyncValue.data(results);
  }

  List<LatLng> _buildGrid(LatLngBounds b) {
    final latSpan = b.north - b.south;
    final lonSpan = b.east - b.west;
    final points = <LatLng>[];
    for (var row = 0; row < _cellsPerAxis; row++) {
      for (var col = 0; col < _cellsPerAxis; col++) {
        // Sample cell centres so points sit inside the viewport rather than
        // on its edges.
        final lat = b.south + latSpan * (row + 0.5) / _cellsPerAxis;
        final lon = b.west + lonSpan * (col + 0.5) / _cellsPerAxis;
        points.add(LatLng(lat, lon));
      }
    }
    return points;
  }

  /// ~1 km coordinate bucket — keeps the cache hit-rate high across small pans
  /// without smearing distinct sea areas together.
  String _key(LatLng p) =>
      '${p.latitude.toStringAsFixed(2)},${p.longitude.toStringAsFixed(2)}';
}
