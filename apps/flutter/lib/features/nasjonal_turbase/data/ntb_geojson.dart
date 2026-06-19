import 'package:latlong2/latlong.dart';

/// Parsers for the GeoJSON carried in a Nasjonal Turbase document's `geojson`
/// field. The field may be a bare geometry (`{type, coordinates}`) or a wrapped
/// `Feature`/`GeometryCollection`; coordinates are GeoJSON order `[lon, lat]`.
///
/// Pure functions, tolerant of malformed input — they never throw, returning
/// `null`/empty so a single bad document can't break a whole viewport load.
class NtbGeoJson {
  NtbGeoJson._();

  /// Extracts a single [LatLng] from a Point geometry (or the first coordinate
  /// of a line, as a fallback for documents that store a route where a point is
  /// expected). Returns `null` when no usable coordinate is found.
  static LatLng? point(Object? geojson) {
    final geom = _geometry(geojson);
    if (geom == null) return null;
    final type = geom['type'];
    final coords = geom['coordinates'];
    if (type == 'Point') return _coord(coords);
    // Degrade: take the first vertex of any line geometry.
    final line = _line(geom);
    return line.isEmpty ? null : line.first;
  }

  /// Flattens a LineString / MultiLineString geometry into a single ordered
  /// list of points (multi-parts are concatenated). Returns an empty list when
  /// there is no line geometry.
  static List<LatLng> line(Object? geojson) {
    final geom = _geometry(geojson);
    if (geom == null) return const [];
    return _line(geom);
  }

  // --- internals ---

  /// Unwraps a Feature / GeometryCollection down to a geometry map.
  static Map<String, dynamic>? _geometry(Object? geojson) {
    if (geojson is! Map) return null;
    final map = Map<String, dynamic>.from(geojson);
    final type = map['type'];
    if (type == 'Feature') return _geometry(map['geometry']);
    if (type == 'GeometryCollection') {
      final geoms = map['geometries'];
      if (geoms is List && geoms.isNotEmpty) return _geometry(geoms.first);
      return null;
    }
    return map;
  }

  static List<LatLng> _line(Map<String, dynamic> geom) {
    final type = geom['type'];
    final coords = geom['coordinates'];
    if (coords is! List) return const [];
    final out = <LatLng>[];
    if (type == 'LineString') {
      for (final c in coords) {
        final p = _coord(c);
        if (p != null) out.add(p);
      }
    } else if (type == 'MultiLineString') {
      for (final part in coords) {
        if (part is! List) continue;
        for (final c in part) {
          final p = _coord(c);
          if (p != null) out.add(p);
        }
      }
    }
    return out;
  }

  static LatLng? _coord(Object? c) {
    if (c is! List || c.length < 2) return null;
    final lon = c[0];
    final lat = c[1];
    if (lon is! num || lat is! num) return null;
    return LatLng(lat.toDouble(), lon.toDouble());
  }
}
