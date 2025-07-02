import 'package:latlong2/latlong.dart';

class DistanceCalculator {
  final Distance _distance;

  DistanceCalculator([Distance? distance])
      : _distance = distance ?? const Distance();

  double calculateDistance(LatLng point1, LatLng point2) {
    return _distance.distance(point1, point2);
  }

  double calculateTotalDistance(List<LatLng> points) {
    if (points.length < 2) return 0;

    double total = 0;
    for (int i = 0; i < points.length - 1; i++) {
      total += calculateDistance(points[i], points[i + 1]);
    }
    return total;
  }
}