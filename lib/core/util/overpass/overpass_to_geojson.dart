import 'dart:convert';

/// Pure Overpass-JSON → GeoJSON converter.
///
/// Overpass returns its own shape — `elements[]` with `geometry: [{lat,
/// lon}]` for ways. We translate to a GeoJSON `FeatureCollection` so
/// downstream code (`VectorLayerFetcher.parseGeoJson`) can read it
/// unchanged, the same way the GML converter feeds the same pipeline.
///
/// Geometry support:
///   - `way` with `geometry: [{lat, lon}, …]`  → LineString
///   - `way` with `tags.area=yes` and a closed ring → Polygon
///   - `node` with `lat`/`lon`                  → Point (skipped for now;
///     trail nodes are rarely useful as features on their own)
///   - `relation`                               → skipped (would need
///     a way-assembly pass)
///
/// Tolerant: malformed JSON yields an empty FeatureCollection rather
/// than throwing. Unknown element kinds skip the element, not the whole
/// document.
class OverpassToGeoJson {
  OverpassToGeoJson._();

  static Map<String, dynamic> convert(String body) {
    final Object? raw;
    try {
      raw = jsonDecode(body);
    } on FormatException {
      return _empty();
    }
    if (raw is! Map<String, dynamic>) return _empty();
    final elements = raw['elements'];
    if (elements is! List) return _empty();

    final features = <Map<String, dynamic>>[];
    var counter = 0;
    for (final el in elements) {
      if (el is! Map<String, dynamic>) continue;
      final type = el['type'];
      if (type != 'way') continue; // nodes/relations skipped for now
      final geomList = el['geometry'];
      if (geomList is! List || geomList.length < 2) continue;
      final coords = <List<double>>[];
      for (final pt in geomList) {
        if (pt is! Map<String, dynamic>) continue;
        final lat = (pt['lat'] as num?)?.toDouble();
        final lon = (pt['lon'] as num?)?.toDouble();
        if (lat == null || lon == null) continue;
        coords.add([lon, lat]); // GeoJSON: [lon, lat]
      }
      if (coords.length < 2) continue;

      final tags = (el['tags'] is Map<String, dynamic>)
          ? Map<String, Object?>.from(el['tags'] as Map)
          : <String, Object?>{};
      final isArea = tags['area'] == 'yes' &&
          coords.length >= 4 &&
          coords.first[0] == coords.last[0] &&
          coords.first[1] == coords.last[1];

      features.add({
        'type': 'Feature',
        'id': 'way/${el['id'] ?? counter++}',
        'geometry': isArea
            ? {
                'type': 'Polygon',
                'coordinates': [coords],
              }
            : {
                'type': 'LineString',
                'coordinates': coords,
              },
        'properties': tags,
      });
    }

    return {
      'type': 'FeatureCollection',
      'features': features,
    };
  }

  static Map<String, dynamic> _empty() => {
        'type': 'FeatureCollection',
        'features': const <Map<String, dynamic>>[],
      };
}
