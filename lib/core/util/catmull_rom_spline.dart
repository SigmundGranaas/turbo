import 'package:latlong2/latlong.dart';

class CatmullRomSpline {
  final List<LatLng> controlPoints;
  final int segments;
  final double tension;

  CatmullRomSpline({
    required this.controlPoints,
    this.segments = 32, // Increased segments for a smoother curve
    this.tension = 0.5,
  });

  List<LatLng> generate() {
    if (controlPoints.length < 2) {
      return controlPoints;
    }

    final List<LatLng> smoothedPoints = [];
    final points = List<LatLng>.from(controlPoints);

    // To handle the endpoints, we duplicate the first and last points.
    points.insert(0, points.first);
    points.add(points.last);

    // Add the very first point.
    smoothedPoints.add(points[1]);

    for (int i = 1; i < points.length - 2; i++) {
      final p0 = points[i - 1];
      final p1 = points[i];
      final p2 = points[i + 1];
      final p3 = points[i + 2];

      for (int j = 1; j <= segments; j++) {
        final t = j / segments;
        smoothedPoints.add(_getPointOnSpline(t, p0, p1, p2, p3));
      }
    }

    return smoothedPoints;
  }

  LatLng _getPointOnSpline(
      double t, LatLng p0, LatLng p1, LatLng p2, LatLng p3) {
    final t2 = t * t;
    final t3 = t2 * t;

    final double lat = tension *
        ((2 * p1.latitude) +
            (-p0.latitude + p2.latitude) * t +
            (2 * p0.latitude - 5 * p1.latitude + 4 * p2.latitude - p3.latitude) *
                t2 +
            (-p0.latitude + 3 * p1.latitude - 3 * p2.latitude + p3.latitude) *
                t3);

    final double lng = tension *
        ((2 * p1.longitude) +
            (-p0.longitude + p2.longitude) * t +
            (2 *
                p0.longitude -
                5 * p1.longitude +
                4 * p2.longitude -
                p3.longitude) *
                t2 +
            (-p0.longitude + 3 * p1.longitude - 3 * p2.longitude + p3.longitude) *
                t3);

    return LatLng(lat, lng);
  }
}