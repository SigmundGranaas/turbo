import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/geo/geo_metrics.dart';

void main() {
  group('GeoMetrics.deviation', () {
    // A simple east-west planned line near the equator-ish latitude where the
    // metre/degree conversion is well-behaved.
    final planned = [
      const LatLng(60.0, 10.0),
      const LatLng(60.0, 10.01),
      const LatLng(60.0, 10.02),
    ];

    test('returns null without enough geometry', () {
      expect(GeoMetrics.deviation([], planned), isNull);
      expect(
        GeoMetrics.deviation([const LatLng(60.0, 10.0)],
            [const LatLng(60.0, 10.0)]),
        isNull,
      );
    });

    test('an exact retrace has ~zero offset and full completion', () {
      final dev = GeoMetrics.deviation(planned, planned)!;
      expect(dev.avgOffsetM, lessThan(1.0));
      expect(dev.maxOffsetM, lessThan(1.0));
      expect(dev.completionFraction, closeTo(1.0, 0.001));
    });

    test('a parallel track offset north reports that offset', () {
      // ~0.0009 deg lat ≈ 100 m north of the planned line.
      final actual = planned
          .map((p) => LatLng(p.latitude + 0.0009, p.longitude))
          .toList();
      final dev = GeoMetrics.deviation(actual, planned)!;
      expect(dev.avgOffsetM, closeTo(100, 15));
      expect(dev.maxOffsetM, greaterThan(80));
    });

    test('turning back halfway caps completion below 1', () {
      // Only reaches the midpoint of the plan.
      final actual = [
        const LatLng(60.0, 10.0),
        const LatLng(60.0, 10.005),
      ];
      final dev = GeoMetrics.deviation(actual, planned)!;
      expect(dev.completionFraction, lessThan(0.6));
      expect(dev.completionFraction, greaterThan(0.2));
    });
  });
}
