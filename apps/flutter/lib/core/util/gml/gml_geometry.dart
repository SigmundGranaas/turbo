import 'package:xml/xml.dart';

import 'gml_axis_order.dart';

/// Extracts GeoJSON-shaped geometry from a GML feature subtree.
///
/// Supported GML types and their GeoJSON mapping:
///   gml:Point            → Point        ([lon, lat])
///   gml:MultiPoint       → MultiPoint   ([[lon, lat], ...])
///   gml:LineString       → LineString   ([[lon, lat], ...])
///   gml:Curve            → LineString   (segments concatenated)
///   gml:MultiCurve       → MultiLineString
///   gml:Polygon          → Polygon      (outer + interior rings)
///   gml:MultiSurface     → MultiPolygon
///
/// Anything else returns `null`, which the converter treats as
/// "feature has no usable geometry — skip it".
class GmlGeometry {
  GmlGeometry._();

  /// Walk [feature] and produce the GeoJSON geometry of the first
  /// recognised `gml:*` element. Coordinate order is normalised to
  /// `[lon, lat]` based on the geometry element's `srsName` (falling
  /// back to [documentSrs]).
  static Map<String, dynamic>? extract(
    XmlElement feature, {
    required String documentSrs,
  }) {
    for (final el in feature.descendantElements) {
      final ns = el.name.namespaceUri ?? '';
      if (!ns.startsWith('http://www.opengis.net/gml')) continue;
      final srs = el.getAttribute('srsName') ?? documentSrs;
      final axisOrder = srs.gmlAxisOrder;
      final geometry = _parse(el, axisOrder);
      if (geometry != null) return geometry;
    }
    return null;
  }

  static Map<String, dynamic>? _parse(XmlElement el, GmlAxisOrder axis) {
    switch (el.localName) {
      case 'Point':
        final c = _readPos(el, axis);
        return c == null ? null : {'type': 'Point', 'coordinates': c};
      case 'MultiPoint':
        final pts = <List<double>>[];
        for (final m in el.findAllElements('Point', namespace: '*')) {
          final c = _readPos(m, axis);
          if (c != null) pts.add(c);
        }
        return pts.isEmpty
            ? null
            : {'type': 'MultiPoint', 'coordinates': pts};
      case 'LineString':
      case 'Curve':
        final line = _readLine(el, axis);
        if (line.length < 2) return null;
        return {'type': 'LineString', 'coordinates': line};
      case 'MultiCurve':
        final parts = <List<List<double>>>[];
        for (final child in el.descendantElements) {
          if (child.localName != 'LineString' && child.localName != 'Curve') {
            continue;
          }
          final line = _readLine(child, axis);
          if (line.length >= 2) parts.add(line);
        }
        return parts.isEmpty
            ? null
            : {'type': 'MultiLineString', 'coordinates': parts};
      case 'Polygon':
        final rings = _readPolygonRings(el, axis);
        return rings.isEmpty
            ? null
            : {'type': 'Polygon', 'coordinates': rings};
      case 'MultiSurface':
      case 'MultiPolygon':
        final polys = <List<List<List<double>>>>[];
        for (final p in el.descendantElements) {
          if (p.localName != 'Polygon') continue;
          final rings = _readPolygonRings(p, axis);
          if (rings.isNotEmpty) polys.add(rings);
        }
        return polys.isEmpty
            ? null
            : {'type': 'MultiPolygon', 'coordinates': polys};
    }
    return null;
  }

  /// Reads the first `gml:pos` (single point) under [el].
  static List<double>? _readPos(XmlElement el, GmlAxisOrder axis) {
    for (final pos in el.findAllElements('pos', namespace: '*')) {
      final pair = _parseTokens(pos.innerText, 2);
      if (pair != null) return _applyAxis(pair[0], pair[1], axis);
    }
    return null;
  }

  /// Reads a polyline from any combination of `gml:posList` and
  /// `gml:pos` descendants under [el]. Concatenates segments if [el] is
  /// a `gml:Curve` made of multiple `LineStringSegment`s.
  static List<List<double>> _readLine(XmlElement el, GmlAxisOrder axis) {
    final out = <List<double>>[];
    for (final d in el.descendantElements) {
      switch (d.localName) {
        case 'posList':
          out.addAll(_parsePosList(d.innerText, axis));
        case 'pos':
          final pair = _parseTokens(d.innerText, 2);
          if (pair != null) out.add(_applyAxis(pair[0], pair[1], axis));
      }
    }
    return out;
  }

  /// Reads outer + holes from a `gml:Polygon`.
  static List<List<List<double>>> _readPolygonRings(
      XmlElement el, GmlAxisOrder axis) {
    final rings = <List<List<double>>>[];
    // gml:exterior → outer ring; gml:interior → hole rings.
    final exterior = el.findElements('exterior', namespace: '*').firstOrNull;
    if (exterior == null) return rings;
    final outerRing = _readLine(exterior, axis);
    if (outerRing.length < 3) return rings;
    rings.add(outerRing);
    for (final hole in el.findElements('interior', namespace: '*')) {
      final inner = _readLine(hole, axis);
      if (inner.length >= 3) rings.add(inner);
    }
    return rings;
  }

  /// Parse a `gml:posList` whitespace-separated coordinate stream.
  ///
  /// Each coordinate is a pair (2D — GML's `srsDimension="3"` for
  /// altitude is uncommon in this dataset and is dropped if encountered:
  /// we only emit X/Y to keep the GeoJSON shape consistent).
  static List<List<double>> _parsePosList(String text, GmlAxisOrder axis) {
    final tokens = text
        .trim()
        .split(RegExp(r'\s+'))
        .where((t) => t.isNotEmpty)
        .toList();
    if (tokens.length < 2) return const [];
    final out = <List<double>>[];
    for (var i = 0; i + 1 < tokens.length; i += 2) {
      final a = double.tryParse(tokens[i]);
      final b = double.tryParse(tokens[i + 1]);
      if (a == null || b == null) continue;
      out.add(_applyAxis(a, b, axis));
    }
    return out;
  }

  static List<double>? _parseTokens(String text, int expectedCount) {
    final tokens = text
        .trim()
        .split(RegExp(r'\s+'))
        .where((t) => t.isNotEmpty)
        .toList();
    if (tokens.length < expectedCount) return null;
    final nums = <double>[];
    for (var i = 0; i < expectedCount; i++) {
      final n = double.tryParse(tokens[i]);
      if (n == null) return null;
      nums.add(n);
    }
    return nums;
  }

  /// Re-orders an `(a, b)` pair into GeoJSON `[lon, lat]`.
  static List<double> _applyAxis(double a, double b, GmlAxisOrder axis) {
    return axis == GmlAxisOrder.latLon ? [b, a] : [a, b];
  }
}
