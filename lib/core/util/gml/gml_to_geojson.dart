import 'package:xml/xml.dart';

import 'gml_geometry.dart';

export 'gml_axis_order.dart' show GmlAxisOrder;

/// Pure GML 3.2.1 → GeoJSON converter.
///
/// Translates a WFS 2.0 `FeatureCollection` GML response into a
/// `Map<String, dynamic>` shaped exactly like a GeoJSON FeatureCollection
/// — so downstream code that already consumes GeoJSON
/// (e.g. `VectorLayerFetcher.parseGeoJson`) can read it unchanged.
///
/// Design constraints:
///  - No I/O. No Flutter imports. No app-specific types in or out.
///  - Tolerant: malformed input returns an empty FeatureCollection rather
///    than throwing. Unknown geometry kinds skip the feature, not the
///    whole collection.
///  - Namespace-agnostic. The converter walks elements by local name and
///    strips `app:`, `gml:`, etc. from property keys so consumers see
///    `rutenavn` not `app:rutenavn`.
///
/// Geometry support is limited to what trail/route datasets actually
/// emit: `gml:LineString`, `gml:MultiCurve`, `gml:Curve`, `gml:Point`,
/// `gml:MultiPoint`, `gml:Polygon`, `gml:MultiSurface`. Extending to
/// further geometry kinds is a [GmlGeometry] change, not a converter
/// change.
class GmlToGeoJson {
  GmlToGeoJson._();

  /// Convert [body] (the raw response text) into a GeoJSON
  /// FeatureCollection map.
  ///
  /// [defaultSrs] is used when neither the feature geometry nor the
  /// document root carries an `srsName`. Defaults to
  /// `urn:ogc:def:crs:EPSG::4326`, which is what Geonorge's WFS
  /// advertises.
  static Map<String, dynamic> convert(
    String body, {
    String defaultSrs = 'urn:ogc:def:crs:EPSG::4326',
  }) {
    final XmlDocument doc;
    try {
      doc = XmlDocument.parse(body);
    } on XmlException {
      return _emptyCollection();
    }

    final root = doc.rootElement;
    final documentSrs = _firstSrs(root) ?? defaultSrs;

    final features = <Map<String, dynamic>>[];
    var counter = 0;
    for (final feature in _featureElements(root)) {
      final geometry = GmlGeometry.extract(
        feature,
        documentSrs: documentSrs,
      );
      if (geometry == null) continue;

      final properties = _extractProperties(feature);
      final id = feature.getAttribute('id',
              namespace: 'http://www.opengis.net/gml/3.2') ??
          feature.getAttribute('id') ??
          'feat-${counter++}';

      features.add({
        'type': 'Feature',
        'id': id,
        'geometry': geometry,
        'properties': properties,
      });
    }

    return {
      'type': 'FeatureCollection',
      'features': features,
    };
  }

  /// Yields every "feature element" inside the FeatureCollection — the
  /// elements directly under `wfs:member` (WFS 2.0) or
  /// `gml:featureMember` (WFS 1.x). A feature element is any child
  /// element whose namespace is **not** gml or wfs (i.e. the application
  /// schema element like `app:Fotrute`).
  static Iterable<XmlElement> _featureElements(XmlElement root) sync* {
    for (final member in root.findAllElements('member', namespace: '*')) {
      yield* member.childElements
          .where((c) => !_isGmlOrWfsNamespace(c.name.namespaceUri));
    }
    for (final member
        in root.findAllElements('featureMember', namespace: '*')) {
      yield* member.childElements
          .where((c) => !_isGmlOrWfsNamespace(c.name.namespaceUri));
    }
  }

  static bool _isGmlOrWfsNamespace(String? uri) {
    if (uri == null) return false;
    return uri.startsWith('http://www.opengis.net/gml') ||
        uri.startsWith('http://www.opengis.net/wfs');
  }

  /// Walks the feature subtree and flattens every leaf scalar into a
  /// `properties` map keyed by local name.
  ///
  /// A "leaf" is an element that has non-empty trimmed text and no child
  /// elements. Wrappers (`app:identifikasjon > app:Identifikasjon > …`)
  /// are descended through. Geometry elements (`gml:*`) and their entire
  /// subtree are skipped — they belong to the geometry, not properties.
  /// On duplicate key the first occurrence wins (document order).
  static Map<String, Object?> _extractProperties(XmlElement feature) {
    final out = <String, Object?>{};

    void visit(XmlElement el) {
      // Skip geometry subtrees entirely — those are handled by
      // GmlGeometry.extract().
      if (_isGmlOrWfsNamespace(el.name.namespaceUri)) return;

      final childElements = el.childElements.toList();
      if (childElements.isEmpty) {
        final text = el.innerText.trim();
        if (text.isEmpty) return;
        // Skip the feature element itself (the outermost call) — it
        // won't have child elements only if the GML is malformed.
        if (el == feature) return;
        out.putIfAbsent(el.localName, () => text);
        return;
      }
      for (final c in childElements) {
        visit(c);
      }
    }

    for (final child in feature.childElements) {
      visit(child);
    }
    return out;
  }

  static String? _firstSrs(XmlElement root) {
    // Some servers attach srsName at the FeatureCollection level. Most
    // attach it per geometry — that's read in GmlGeometry.extract.
    return root.getAttribute('srsName');
  }

  static Map<String, dynamic> _emptyCollection() => {
        'type': 'FeatureCollection',
        'features': const <Map<String, dynamic>>[],
      };
}

