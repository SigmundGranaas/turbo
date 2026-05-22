import 'package:latlong2/latlong.dart';

/// Minimal GeoJSON-derived feature flattened for rendering on a flutter_map
/// layer. We support LineString / MultiLineString and Polygon / MultiPolygon
/// since those are the only geometry types we currently surface (trails are
/// lines, MetAlerts and Naturbase areas are polygons).
class VectorFeature {
  /// Stable identifier (GeoJSON `id` field or property fallback). Used for
  /// hit-testing.
  final String id;

  /// Logical kind of geometry: 'line' or 'polygon'. Drives which flutter_map
  /// layer the feature is added to.
  final VectorGeometryKind kind;

  /// One or more rings — each ring is a connected sequence of LatLng. Single
  /// LineString and Polygon yield a list of length 1; Multi* yield more.
  final List<List<LatLng>> rings;

  /// Raw property bag from the source GeoJSON. The feature-detail sheet
  /// surfaces these directly.
  final Map<String, Object?> properties;

  const VectorFeature({
    required this.id,
    required this.kind,
    required this.rings,
    required this.properties,
  });

  /// GeoJSON representation suitable for round-tripping through the
  /// persistent cache. The persistent store writes the FeatureCollection
  /// as a JSON string; the fetcher's GeoJSON parser inverts this on read.
  Map<String, Object?> toGeoJson() {
    return {
      'type': 'Feature',
      'id': id,
      'properties': properties,
      'geometry': _geometryJson(),
    };
  }

  Map<String, Object?> _geometryJson() {
    List<List<List<double>>> ringsAsCoords() => [
          for (final ring in rings)
            [
              for (final p in ring) [p.longitude, p.latitude]
            ]
        ];
    if (kind == VectorGeometryKind.line) {
      if (rings.length == 1) {
        return {
          'type': 'LineString',
          'coordinates': [
            for (final p in rings.first) [p.longitude, p.latitude]
          ],
        };
      }
      return {
        'type': 'MultiLineString',
        'coordinates': ringsAsCoords(),
      };
    }
    if (rings.length == 1) {
      return {
        'type': 'Polygon',
        'coordinates': [
          [for (final p in rings.first) [p.longitude, p.latitude]]
        ],
      };
    }
    return {
      'type': 'MultiPolygon',
      'coordinates': [
        for (final ring in rings)
          [
            [for (final p in ring) [p.longitude, p.latitude]]
          ],
      ],
    };
  }
}

enum VectorGeometryKind { line, polygon }
