import 'package:latlong2/latlong.dart';
import 'package:xml/xml.dart';

import '../models/elevation_stats.dart';
import '../models/saved_path.dart';

/// Parses a GPX 1.1 document into one [SavedPath] per `<trk>` element.
///
/// Behavior:
/// * Title comes from `<trk><name>` if present, otherwise `<metadata><name>`,
///   otherwise a synthetic "Imported track".
/// * Description comes from `<trk><desc>` or `<metadata><desc>`.
/// * Points concatenate all `<trkseg><trkpt>` children of the track.
/// * Elevations are read from `<ele>` when present; missing elevations
///   become `NaN` so positional alignment with [SavedPath.points] is preserved.
/// * Distance is computed via haversine.
/// * Ascent / descent are derived via [ElevationStats] when at least one
///   elevation is present.
///
/// Throws [FormatException] on malformed XML.
List<SavedPath> parseGpx(String xml) {
  XmlDocument doc;
  try {
    doc = XmlDocument.parse(xml);
  } catch (e) {
    throw FormatException('Invalid GPX: $e');
  }

  final root = doc.rootElement;
  if (root.localName != 'gpx') {
    throw const FormatException('Not a GPX document — root is not <gpx>');
  }

  final metadataName =
      root.getElement('metadata')?.getElement('name')?.innerText.trim();
  final metadataDesc =
      root.getElement('metadata')?.getElement('desc')?.innerText.trim();

  final result = <SavedPath>[];
  for (final trk in root.findElements('trk')) {
    final trackName = trk.getElement('name')?.innerText.trim();
    final trackDesc = trk.getElement('desc')?.innerText.trim();

    final points = <LatLng>[];
    final elevations = <double>[];
    var anyElevation = false;

    for (final seg in trk.findElements('trkseg')) {
      for (final trkpt in seg.findElements('trkpt')) {
        final lat = double.tryParse(trkpt.getAttribute('lat') ?? '');
        final lon = double.tryParse(trkpt.getAttribute('lon') ?? '');
        if (lat == null || lon == null) continue;
        points.add(LatLng(lat, lon));
        final eleText = trkpt.getElement('ele')?.innerText.trim();
        if (eleText != null) {
          final ele = double.tryParse(eleText);
          if (ele != null) {
            elevations.add(ele);
            anyElevation = true;
            continue;
          }
        }
        elevations.add(double.nan);
      }
    }

    if (points.length < 2) continue;

    final distance = _haversineTotal(points);
    final title = (trackName?.isNotEmpty == true)
        ? trackName!
        : (metadataName?.isNotEmpty == true ? metadataName! : 'Imported track');
    final description = (trackDesc?.isNotEmpty == true)
        ? trackDesc
        : (metadataDesc?.isNotEmpty == true ? metadataDesc : null);

    final stats = anyElevation
        ? ElevationStats.fromSamples(
            elevations.map((e) => e.isNaN ? null : e).toList())
        : ElevationStats.zero;

    result.add(SavedPath(
      title: title,
      description: description,
      points: points,
      distance: distance,
      elevations: anyElevation ? elevations : null,
      ascent: anyElevation ? stats.ascent : null,
      descent: anyElevation ? stats.descent : null,
    ));
  }

  return result;
}

double _haversineTotal(List<LatLng> points) {
  const distance = Distance();
  var total = 0.0;
  for (var i = 1; i < points.length; i++) {
    total += distance.distance(points[i - 1], points[i]);
  }
  return total;
}
