import 'package:latlong2/latlong.dart';

/// Coarse-grained kind of geometry an activity occupies. Mirrors the
/// server's `ActivityGeometryKind` enum one-to-one.
enum ActivityGeometryKind { point, lineString, polygon }

/// Wire-format geometry. The server sends WKT; this app keeps WKT around
/// for round-trip but also exposes typed Dart-side accessors for the
/// common shapes the map layer needs.
class ActivityGeometry {
  final ActivityGeometryKind kind;
  final String wkt;
  final List<LatLng> _coordinates;

  ActivityGeometry._(this.kind, this.wkt, this._coordinates);

  List<LatLng> get coordinates => List.unmodifiable(_coordinates);

  /// First coordinate (or null if empty). Convenience for marker layers.
  LatLng? get firstPoint => _coordinates.isEmpty ? null : _coordinates.first;

  /// Parse the server-emitted WKT + geometryKind pair. WKT is simple
  /// enough that we don't pull in a heavyweight parser for the shapes we
  /// care about (POINT, LINESTRING, POLYGON).
  factory ActivityGeometry.fromServer({
    required String wkt,
    required String geometryKind,
  }) {
    final coords = _parseWkt(wkt);
    final kind = switch (geometryKind.toUpperCase()) {
      'POINT' => ActivityGeometryKind.point,
      'LINESTRING' => ActivityGeometryKind.lineString,
      'POLYGON' => ActivityGeometryKind.polygon,
      _ => throw FormatException('Unsupported geometry kind: $geometryKind'),
    };
    return ActivityGeometry._(kind, wkt, coords);
  }

  /// Build a WKT POINT from a [LatLng]. Used by the create flow when
  /// posting to a kind's typed endpoint.
  static String pointWkt(LatLng p) => 'POINT(${p.longitude} ${p.latitude})';

  /// Build a typed point geometry from a [LatLng] directly — skips the
  /// WKT-then-parse round trip that `fromServer` would do.
  factory ActivityGeometry.fromPoint(LatLng p) => ActivityGeometry._(
        ActivityGeometryKind.point,
        pointWkt(p),
        [p],
      );

  /// Build a typed linestring geometry from a list of [LatLng]s. Used by
  /// the route-drawing flow before any server round-trip.
  factory ActivityGeometry.fromRoute(List<LatLng> points) {
    final wkt = 'LINESTRING(${points.map((p) => '${p.longitude} ${p.latitude}').join(', ')})';
    return ActivityGeometry._(
      ActivityGeometryKind.lineString,
      wkt,
      List<LatLng>.from(points),
    );
  }

  static List<LatLng> _parseWkt(String wkt) {
    final trimmed = wkt.trim();
    final openParen = trimmed.indexOf('(');
    if (openParen < 0) return const [];
    final body = trimmed.substring(openParen).replaceAll('(', '').replaceAll(')', '');
    final pairs = body.split(',');
    final out = <LatLng>[];
    for (final pair in pairs) {
      final p = pair.trim().split(RegExp(r'\s+'));
      if (p.length < 2) continue;
      final lon = double.tryParse(p[0]);
      final lat = double.tryParse(p[1]);
      if (lon == null || lat == null) continue;
      out.add(LatLng(lat, lon));
    }
    return out;
  }
}
