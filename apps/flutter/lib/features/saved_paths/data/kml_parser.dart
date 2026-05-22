import 'package:latlong2/latlong.dart';
import 'package:xml/xml.dart';

import '../models/elevation_stats.dart';
import '../models/saved_path.dart';

/// Minimal KML 2.2 parser — extracts `<LineString><coordinates>` blocks under
/// `<Placemark>` elements. Each Placemark with a LineString becomes one path.
///
/// KML coordinates are `lon,lat[,alt]` space-separated. Other geometry types
/// (Point, Polygon, MultiGeometry except line-only) are skipped.
List<SavedPath> parseKml(String xml) {
  XmlDocument doc;
  try {
    doc = XmlDocument.parse(xml);
  } catch (e) {
    throw FormatException('Invalid KML: $e');
  }

  final placemarks =
      doc.findAllElements('Placemark', namespace: '*').toList();
  final out = <SavedPath>[];

  for (final pm in placemarks) {
    final name = pm.getElement('name', namespace: '*')?.innerText.trim();
    final desc =
        pm.getElement('description', namespace: '*')?.innerText.trim();
    for (final ls in pm.findElements('LineString', namespace: '*')) {
      final coordsText =
          ls.getElement('coordinates', namespace: '*')?.innerText.trim();
      if (coordsText == null || coordsText.isEmpty) continue;
      final parsed = _parseCoords(coordsText);
      if (parsed.$1.length < 2) continue;
      final points = parsed.$1;
      final elevations = parsed.$2;
      final hasElevations = elevations.any((e) => e != null);
      final stats = hasElevations
          ? ElevationStats.fromSamples(elevations)
          : ElevationStats.zero;
      out.add(SavedPath(
        title: (name?.isNotEmpty == true) ? name! : 'Imported track',
        description: (desc?.isNotEmpty == true) ? desc : null,
        points: points,
        distance: _haversineTotal(points),
        elevations: hasElevations
            ? elevations.map((e) => e ?? double.nan).toList()
            : null,
        ascent: hasElevations ? stats.ascent : null,
        descent: hasElevations ? stats.descent : null,
      ));
    }
  }
  return out;
}

(List<LatLng>, List<double?>) _parseCoords(String text) {
  final points = <LatLng>[];
  final elevations = <double?>[];
  for (final triple in text.split(RegExp(r'\s+'))) {
    if (triple.isEmpty) continue;
    final parts = triple.split(',');
    if (parts.length < 2) continue;
    final lon = double.tryParse(parts[0]);
    final lat = double.tryParse(parts[1]);
    if (lon == null || lat == null) continue;
    points.add(LatLng(lat, lon));
    if (parts.length >= 3) {
      elevations.add(double.tryParse(parts[2]));
    } else {
      elevations.add(null);
    }
  }
  return (points, elevations);
}

double _haversineTotal(List<LatLng> points) {
  const distance = Distance();
  var total = 0.0;
  for (var i = 1; i < points.length; i++) {
    total += distance.distance(points[i - 1], points[i]);
  }
  return total;
}
