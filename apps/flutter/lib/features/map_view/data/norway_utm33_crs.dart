import 'dart:math' as math;

import 'package:flutter_map/flutter_map.dart';
import 'package:proj4dart/proj4dart.dart' as proj4;

/// Which projection the map is currently rendering in.
///
/// The app's default is Web Mercator (EPSG:3857) like almost every web map.
/// When Norwegian topo is the sole base layer we switch the whole map to
/// UTM33 (EPSG:25833), because Kartverket only publishes the high-detail topo
/// raster in that projection — its Web Mercator cache stops at native zoom 18
/// (~1:2 132), whereas the UTM33 grid reaches ~1:295, giving ~3 extra levels
/// of sharp detail (matching norgeskart.no).
enum MapProjection { webMercator, utm33 }

/// OGC standard pixel size in metres (0.28 mm), used to turn WMTS scale
/// denominators into metres-per-pixel resolutions.
const double _kPixelSpanMetres = 0.00028;

/// Scale denominators for Kartverket's `utm33n` TileMatrixSet, levels 0–18,
/// taken verbatim from the WMTS GetCapabilities document. Each level is an
/// exact halving of the previous one (z0 / z18 == 2^18).
const List<double> _utm33ScaleDenominators = <double>[
  77371428.57142857, // 00
  38685714.28571428, // 01
  19342857.14285714, // 02
  9671428.57142857, // 03
  4835714.285714285, // 04
  2417857.1428571427, // 05
  1208928.5714285714, // 06
  604464.2857142857, // 07
  302232.1428571428, // 08
  151116.0714285714, // 09
  75558.0357142857, // 10
  37779.0178571429, // 11
  18889.5089285714, // 12
  9444.7544642857, // 13
  4722.3772321429, // 14
  2361.1886160714, // 15
  1180.5943080357, // 16
  590.2971540179, // 17
  295.1485770089, // 18
];

/// Metres-per-pixel for each `utm33n` zoom level (0–18).
final List<double> norwayUtm33Resolutions = _utm33ScaleDenominators
    .map((s) => s * _kPixelSpanMetres)
    .toList(growable: false);

/// Resolution of the deepest native level (z18), ~0.0826 m/px.
final double _utm33Res18 = norwayUtm33Resolutions.last;

/// Top-left corner (origin) of the `utm33n` grid in EPSG:25833 easting,
/// northing — shared by every zoom level.
const math.Point<double> _utm33Origin = math.Point(-2500000.0, 9045984.0);

/// Highest native zoom level the `utm33n` topo grid serves.
const int utm33MaxNativeZoom = 18;

proj4.Projection _resolveUtm33Projection() {
  // proj4dart ships a handful of well-known EPSG defs but not EPSG:25833, so
  // register it on first use. `Projection.add` is idempotent-friendly: we
  // only add when it isn't already known.
  return proj4.Projection.get('EPSG:25833') ??
      proj4.Projection.add(
        'EPSG:25833',
        '+proj=utm +zone=33 +ellps=GRS80 '
            '+towgs84=0,0,0,0,0,0,0 +units=m +no_defs',
      );
}

/// The map CRS for Kartverket's UTM33 topo grid (EPSG:25833).
///
/// Grid parameters (origin, 256 px tiles, 19 levels) mirror the WMTS
/// `utm33n` TileMatrixSet so flutter_map requests exactly the `TileCol`/
/// `TileRow` the server expects.
final Crs norwayUtm33Crs = Proj4Crs.fromFactory(
  code: 'EPSG:25833',
  proj4Projection: _resolveUtm33Projection(),
  origins: const <math.Point<double>>[_utm33Origin],
  resolutions: norwayUtm33Resolutions,
);

/// The flutter_map [Crs] for a given [MapProjection].
Crs crsForProjection(MapProjection projection) =>
    projection == MapProjection.utm33 ? norwayUtm33Crs : const Epsg3857();

const double _webMercatorRes0 = 156543.03392804097; // m/px at equator, z0

/// Ground resolution (metres-per-pixel as seen on the ground) at [zoom] for
/// [projection] at the given [latitude]. Used to keep the visible scale
/// roughly constant when the map switches projections.
double _groundResolution(
    MapProjection projection, double zoom, double latitude) {
  if (projection == MapProjection.utm33) {
    // UTM is conformal with a scale factor near 1 across Norway, so the
    // projected resolution is effectively the ground resolution.
    return _utm33Res18 * math.pow(2, utm33MaxNativeZoom - zoom);
  }
  // Web Mercator stretches by 1/cos(lat); fold that in for a true ground value.
  return (_webMercatorRes0 / math.pow(2, zoom)) *
      math.cos(latitude * math.pi / 180.0);
}

double _zoomForGroundResolution(
    MapProjection projection, double resolution, double latitude) {
  if (projection == MapProjection.utm33) {
    return utm33MaxNativeZoom - _log2(resolution / _utm33Res18);
  }
  final cosLat = math.cos(latitude * math.pi / 180.0);
  return _log2(_webMercatorRes0 * cosLat / resolution);
}

double _log2(num x) => math.log(x) / math.ln2;

/// Translate a [zoom] level from one projection to another so the map stays
/// at the same visible scale (at [latitude]) when the base projection flips.
///
/// A given integer zoom means very different things in the two grids — at
/// Norwegian latitudes UTM33 z16 ≈ Web Mercator z18 — so without this a base
/// layer toggle would jump the camera. Returns [zoom] unchanged when the
/// projection is the same.
double convertZoomBetweenProjections({
  required double zoom,
  required double latitude,
  required MapProjection from,
  required MapProjection to,
}) {
  if (from == to) return zoom;
  final resolution = _groundResolution(from, zoom, latitude);
  return _zoomForGroundResolution(to, resolution, latitude);
}
