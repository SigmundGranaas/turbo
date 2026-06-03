import 'dart:math' as math;

import 'package:flutter_map/flutter_map.dart' show LatLngBounds;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/geo/geo_path.dart';

/// Bounds covering a route plus a buffer — for "download the map along this
/// route". Region downloads are bounding-box based, so the corridor is the
/// path's bounding box widened by [paddingMeters] on every side so the user
/// gets the surroundings (and a little slack for going off-route), not a sliver
/// hugging the line.
LatLngBounds corridorBounds(GeoPath path, {double paddingMeters = 1000}) {
  final box = path.bounds;
  final midLat = (box.south + box.north) / 2;
  final latPad = paddingMeters / 111320.0;
  final cosLat = math.max(0.1, math.cos(midLat * math.pi / 180).abs());
  final lngPad = paddingMeters / (111320.0 * cosLat);
  return LatLngBounds(
    LatLng(box.south - latPad, box.west - lngPad),
    LatLng(box.north + latPad, box.east + lngPad),
  );
}
