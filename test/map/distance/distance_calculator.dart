import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/widgets/map/measuring/distance_calculator.dart' as calc;

void main() {
  group('DistanceCalculator', () {
    late calc.DistanceCalculator calculator;

    setUp(() {
      calculator = calc.DistanceCalculator();
    });

    test('calculates realistic distances between points', () {
      // Test with known real-world distances
      const tokyo = LatLng(35.6762, 139.6503);
      const osaka = LatLng(34.6937, 135.5023);

      final distance = calculator.calculateDistance(tokyo, osaka);

      // Known distance between Tokyo and Osaka is roughly 400km
      expect(distance / 1000, closeTo(400, 10));
    });

    test('calculates total distance for route', () {
      const points = [
        LatLng(0, 0),      // Origin
        LatLng(0, 1),      // 111km east
        LatLng(1, 1),      // 111km north
      ];

      final totalDistance = calculator.calculateTotalDistance(points);

      // Expected: roughly 222km (111km + 111km)
      expect(totalDistance / 1000, closeTo(222, 5));
    });
  });
}