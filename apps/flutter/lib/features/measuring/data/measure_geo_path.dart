import 'package:turbo/core/geo/geo_path.dart';

import '../models/measure_point.dart';

/// Bridge from an in-progress measurement to the shared [GeoPath]. The caller
/// passes the running total distance (the measuring state already tracks it).
GeoPath measurePointsToGeoPath(
  List<MeasurePoint> measurePoints, {
  required double distanceM,
}) {
  return GeoPath(
    points: measurePoints.map((p) => p.point).toList(growable: false),
    distanceM: distanceM,
    source: GeoPathSource.measure,
  );
}
