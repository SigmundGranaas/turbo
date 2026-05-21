import 'dart:convert';

import 'package:latlong2/latlong.dart';

import '../models/elevation_stats.dart';
import '../models/saved_path.dart';

/// Parses a GeoJSON document into one [SavedPath] per LineString.
///
/// Supports:
/// * `FeatureCollection` of `Feature`s with `LineString` or `MultiLineString`
///   geometries (each line within a MultiLineString becomes one path).
/// * Bare `Feature` with a LineString.
/// * Bare `LineString` geometry.
///
/// Properties consulted: `title` (or `name`), `description`. Altitudes are
/// read from the 3rd element of each coordinate tuple per RFC 7946 §3.1.1.
///
/// Throws [FormatException] on malformed JSON or unsupported geometry types.
List<SavedPath> parseGeoJson(String jsonStr) {
  dynamic json;
  try {
    json = jsonDecode(jsonStr);
  } catch (e) {
    throw FormatException('Invalid GeoJSON: $e');
  }
  if (json is! Map<String, dynamic>) {
    throw const FormatException('GeoJSON root must be an object');
  }

  final out = <SavedPath>[];
  switch (json['type']) {
    case 'FeatureCollection':
      final features = json['features'];
      if (features is List) {
        for (final f in features) {
          if (f is Map<String, dynamic>) _addFromFeature(f, out);
        }
      }
    case 'Feature':
      _addFromFeature(json, out);
    case 'LineString':
    case 'MultiLineString':
      _addFromGeometry(json, const {}, out);
    default:
      throw FormatException(
          'Unsupported GeoJSON type: ${json['type']}');
  }
  return out;
}

void _addFromFeature(
    Map<String, dynamic> feature, List<SavedPath> out) {
  final geom = feature['geometry'];
  final props = feature['properties'];
  final propsMap = props is Map<String, dynamic> ? props : const <String, dynamic>{};
  if (geom is Map<String, dynamic>) {
    _addFromGeometry(geom, propsMap, out);
  }
}

void _addFromGeometry(Map<String, dynamic> geom,
    Map<String, dynamic> props, List<SavedPath> out) {
  switch (geom['type']) {
    case 'LineString':
      final line = _coordsAsLine(geom['coordinates']);
      if (line != null) {
        final path = _makePath(line.$1, line.$2, props);
        if (path != null) out.add(path);
      }
    case 'MultiLineString':
      final coords = geom['coordinates'];
      if (coords is List) {
        for (final l in coords) {
          final line = _coordsAsLine(l);
          if (line != null) {
            final path = _makePath(line.$1, line.$2, props);
            if (path != null) out.add(path);
          }
        }
      }
    default:
      // Other geometry types are silently skipped — a Polygon or Point isn't a
      // track and shouldn't break import of mixed FeatureCollections.
      break;
  }
}

(List<LatLng>, List<double?>)? _coordsAsLine(dynamic coords) {
  if (coords is! List) return null;
  final points = <LatLng>[];
  final elevations = <double?>[];
  for (final c in coords) {
    if (c is! List || c.length < 2) continue;
    final lng = (c[0] as num).toDouble();
    final lat = (c[1] as num).toDouble();
    points.add(LatLng(lat, lng));
    if (c.length >= 3 && c[2] is num) {
      elevations.add((c[2] as num).toDouble());
    } else {
      elevations.add(null);
    }
  }
  if (points.length < 2) return null;
  return (points, elevations);
}

SavedPath? _makePath(List<LatLng> points, List<double?> elevations,
    Map<String, dynamic> props) {
  if (points.length < 2) return null;
  final title = (props['title'] as String?)?.trim().isNotEmpty == true
      ? props['title'] as String
      : ((props['name'] as String?)?.trim().isNotEmpty == true
          ? props['name'] as String
          : 'Imported track');
  final description = (props['description'] as String?)?.trim().isNotEmpty == true
      ? props['description'] as String
      : null;
  final distance = _haversineTotal(points);
  final hasElevations = elevations.any((e) => e != null);
  final stats =
      hasElevations ? ElevationStats.fromSamples(elevations) : ElevationStats.zero;
  return SavedPath(
    title: title,
    description: description,
    points: points,
    distance: distance,
    elevations: hasElevations
        ? elevations.map((e) => e ?? double.nan).toList()
        : null,
    ascent: hasElevations ? stats.ascent : null,
    descent: hasElevations ? stats.descent : null,
  );
}

double _haversineTotal(List<LatLng> points) {
  const distance = Distance();
  var total = 0.0;
  for (var i = 1; i < points.length; i++) {
    total += distance.distance(points[i - 1], points[i]);
  }
  return total;
}
