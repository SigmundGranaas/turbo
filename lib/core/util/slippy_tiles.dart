import 'dart:math' as math;

/// Slippy-map tile coordinate. `z` is the zoom level, `x`/`y` are the
/// integer tile indices in EPSG:3857 (Web Mercator).
class SlippyTile {
  final int z;
  final int x;
  final int y;
  const SlippyTile(this.z, this.x, this.y);

  /// Geographic bounds covered by this tile, in WGS84 degrees.
  SlippyTileBounds get bounds {
    final n = 1 << z;
    final lonMin = x / n * 360.0 - 180.0;
    final lonMax = (x + 1) / n * 360.0 - 180.0;
    final latMax = _tileLat(y, n);
    final latMin = _tileLat(y + 1, n);
    return SlippyTileBounds(
      north: latMax,
      south: latMin,
      east: lonMax,
      west: lonMin,
    );
  }

  static double _tileLat(int y, int n) {
    final s = math.pi - 2.0 * math.pi * y / n;
    return 180.0 / math.pi * math.atan(0.5 * (math.exp(s) - math.exp(-s)));
  }

  @override
  bool operator ==(Object other) =>
      other is SlippyTile && other.z == z && other.x == x && other.y == y;

  @override
  int get hashCode => Object.hash(z, x, y);
}

/// Geographic bounding box of a slippy tile.
class SlippyTileBounds {
  final double north;
  final double south;
  final double east;
  final double west;
  const SlippyTileBounds({
    required this.north,
    required this.south,
    required this.east,
    required this.west,
  });
}

/// Lists every slippy tile at [zoom] that intersects the WGS84 bbox
/// `[minLat..maxLat] × [minLon..maxLon]`. Off-world tiles are clamped to
/// the valid `[0, 2^z - 1]` index range.
///
/// Used wherever a feature-bounded fetch needs to be diced into a
/// reusable tile grid — currently the vector-layer repository's
/// cache-keying step. The offline-region downloader also walks a tile
/// grid but goes through flutter_map's `Epsg3857.latLngToOffset`
/// because it needs multi-zoom enumeration in one call; this helper
/// is single-zoom by design.
List<SlippyTile> tilesCovering({
  required int zoom,
  required double minLat,
  required double minLon,
  required double maxLat,
  required double maxLon,
}) {
  final tl = _lonLatToTileIndex(minLon, maxLat, zoom);
  final br = _lonLatToTileIndex(maxLon, minLat, zoom);
  final maxIndex = (1 << zoom) - 1;
  final x0 = math.min(tl.x, br.x);
  final x1 = math.max(tl.x, br.x);
  final y0 = math.min(tl.y, br.y);
  final y1 = math.max(tl.y, br.y);
  final out = <SlippyTile>[];
  for (var x = x0; x <= x1; x++) {
    for (var y = y0; y <= y1; y++) {
      if (x < 0 || y < 0 || x > maxIndex || y > maxIndex) continue;
      out.add(SlippyTile(zoom, x, y));
    }
  }
  return out;
}

SlippyTile _lonLatToTileIndex(double lon, double lat, int z) {
  final n = 1 << z;
  final x = ((lon + 180.0) / 360.0 * n).floor();
  final latRad = lat * math.pi / 180.0;
  final y = ((1.0 -
              (math.log(math.tan(latRad) + 1 / math.cos(latRad)) / math.pi)) /
          2.0 *
          n)
      .floor();
  return SlippyTile(z, x.clamp(0, n - 1), y.clamp(0, n - 1));
}
