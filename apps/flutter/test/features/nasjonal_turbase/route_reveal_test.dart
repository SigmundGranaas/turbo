import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/nasjonal_turbase/util/route_reveal.dart';

void main() {
  // A simple eastbound line at the equator; three evenly spaced vertices so
  // cumulative distances are 0, d, 2d.
  final line = [
    const LatLng(0, 0),
    const LatLng(0, 1),
    const LatLng(0, 2),
  ];

  group('cumulativeDistances', () {
    test('is monotonic, starts at 0, with equal segments equal', () {
      final c = RouteReveal.cumulativeDistances(line);
      expect(c.length, 3);
      expect(c.first, 0);
      expect(c[1], greaterThan(0));
      expect(c[2], closeTo(2 * c[1], c[1] * 1e-6));
    });

    test('empty for fewer than two points', () {
      expect(RouteReveal.cumulativeDistances(const []), isEmpty);
      expect(RouteReveal.cumulativeDistances([const LatLng(1, 1)]), isEmpty);
    });
  });

  group('revealPolyline', () {
    test('t=0 yields just the start', () {
      expect(RouteReveal.revealPolyline(line, 0), [line.first]);
    });

    test('t=1 yields the full line', () {
      expect(RouteReveal.revealPolyline(line, 1), line);
    });

    test('t=0.5 reaches the midpoint vertex', () {
      final revealed = RouteReveal.revealPolyline(line, 0.5);
      expect(revealed.last.longitude, closeTo(1.0, 1e-6));
      expect(revealed.last.latitude, closeTo(0.0, 1e-6));
    });

    test('t=0.25 interpolates a tip partway along the first segment', () {
      final revealed = RouteReveal.revealPolyline(line, 0.25);
      expect(revealed.length, 2); // start + interpolated tip
      expect(revealed.last.longitude, closeTo(0.5, 1e-3));
    });

    test('clamps out-of-range t', () {
      expect(RouteReveal.revealPolyline(line, -1), [line.first]);
      expect(RouteReveal.revealPolyline(line, 5), line);
    });

    test('degenerate inputs do not throw', () {
      expect(RouteReveal.revealPolyline(const [], 0.5), isEmpty);
      expect(RouteReveal.revealPolyline([const LatLng(1, 1)], 0.5),
          [const LatLng(1, 1)]);
    });
  });

  group('pointAt', () {
    test('returns the moving head', () {
      final head = RouteReveal.pointAt(line, 0.5);
      expect(head, isNotNull);
      expect(head!.longitude, closeTo(1.0, 1e-6));
    });
  });
}
