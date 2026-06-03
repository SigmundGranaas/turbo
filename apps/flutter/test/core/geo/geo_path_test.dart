import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/geo/geo_metrics.dart';
import 'package:turbo/core/geo/geo_path.dart';

void main() {
  group('GeoMetrics', () {
    test('pathLengthMeters is 0 for < 2 points', () {
      expect(GeoMetrics.pathLengthMeters(const []), 0);
      expect(GeoMetrics.pathLengthMeters([const LatLng(60, 10)]), 0);
    });

    test('pathLengthMeters sums segment lengths', () {
      final pts = [
        const LatLng(60.0, 10.0),
        const LatLng(60.0, 10.001),
        const LatLng(60.0, 10.002),
      ];
      final total = GeoMetrics.pathLengthMeters(pts);
      final seg = GeoMetrics.distanceMeters(pts[0], pts[1]);
      expect(total, closeTo(seg * 2, 1e-6));
      expect(total, greaterThan(0));
    });

    test('bearingDegrees normalises to 0..360', () {
      final b = GeoMetrics.bearingDegrees(
          const LatLng(60, 10), const LatLng(61, 10));
      expect(b, inInclusiveRange(0, 360));
      expect(b, closeTo(0, 1)); // due north
    });

    test('ascentDescent ignores nulls and NaN', () {
      final r = GeoMetrics.ascentDescent([100, null, 110, 105, double.nan, 115]);
      // 100→110 (+10), 110→105 (-5), 105→115 (+10) => ascent 20, descent 5
      expect(r.ascent, closeTo(20, 1e-9));
      expect(r.descent, closeTo(5, 1e-9));
    });

    test('progress: start, midpoint, end', () {
      final path = [
        const LatLng(60.0, 10.0),
        const LatLng(60.0, 10.01),
      ];
      final total = GeoMetrics.pathLengthMeters(path);

      final atStart = GeoMetrics.progress(path, path.first)!;
      expect(atStart.distanceAlongM, closeTo(0, 1));
      expect(atStart.fraction, closeTo(0, 1e-3));

      final atEnd = GeoMetrics.progress(path, path.last)!;
      expect(atEnd.remainingM, closeTo(0, 1));
      expect(atEnd.fraction, closeTo(1, 1e-3));

      final mid = GeoMetrics.progress(path, const LatLng(60.0, 10.005))!;
      expect(mid.distanceAlongM, closeTo(total / 2, total * 0.02));
      expect(mid.offRouteM, closeTo(0, 1));
    });

    test('progress: off-route point reports perpendicular distance', () {
      final path = [
        const LatLng(60.0, 10.0),
        const LatLng(60.0, 10.01),
      ];
      // ~111 m north of the line midpoint.
      final p = GeoMetrics.progress(path, const LatLng(60.001, 10.005))!;
      expect(p.offRouteM, greaterThan(50));
    });

    test('progress: null for degenerate path', () {
      expect(GeoMetrics.progress(const [], const LatLng(60, 10)), isNull);
    });
  });

  group('GeoPath', () {
    test('fromPoints computes distance', () {
      final p = GeoPath.fromPoints(
        const [LatLng(60.0, 10.0), LatLng(60.0, 10.001)],
        source: GeoPathSource.measure,
      );
      expect(p.distanceM, greaterThan(0));
      expect(p.isEmpty, isFalse);
    });

    test('isEmpty for < 2 points', () {
      const p = GeoPath(points: [], distanceM: 0, source: GeoPathSource.route);
      expect(p.isEmpty, isTrue);
    });

    test('bounds covers all points', () {
      final p = GeoPath.fromPoints(
        const [LatLng(60.0, 10.0), LatLng(61.0, 11.0)],
        source: GeoPathSource.saved,
      );
      final b = p.bounds;
      expect(b.south, closeTo(60.0, 1e-9));
      expect(b.north, closeTo(61.0, 1e-9));
      expect(b.west, closeTo(10.0, 1e-9));
      expect(b.east, closeTo(11.0, 1e-9));
    });

    test('copyWith overrides selectively', () {
      final p = GeoPath.fromPoints(
        const [LatLng(60.0, 10.0), LatLng(60.0, 10.001)],
        source: GeoPathSource.recording,
      );
      final q = p.copyWith(ascentM: 42);
      expect(q.ascentM, 42);
      expect(q.distanceM, p.distanceM);
      expect(q.source, GeoPathSource.recording);
    });
  });
}
