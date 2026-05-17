import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/saved_paths/api.dart' show ElevationStats;

void main() {
  group('ElevationStats.fromSamples', () {
    test('returns zero when fewer than 2 valid samples', () {
      expect(ElevationStats.fromSamples([]).ascent, 0);
      expect(ElevationStats.fromSamples([null]).ascent, 0);
      expect(ElevationStats.fromSamples([100.0]).descent, 0);
    });

    test('flat series has zero ascent and zero descent', () {
      final stats = ElevationStats.fromSamples(List.filled(20, 100.0));
      expect(stats.ascent, 0);
      expect(stats.descent, 0);
    });

    test('monotonic climb accumulates ascent only', () {
      // 100 → 200 over 11 evenly-spaced samples.
      final series = List<double?>.generate(
          11, (i) => 100.0 + i * 10.0);
      final stats = ElevationStats.fromSamples(series);
      expect(stats.descent, 0);
      // The smoother dampens the edges, so we accept any value in a reasonable
      // band rather than asserting exactly 100 m.
      expect(stats.ascent, greaterThanOrEqualTo(70));
      expect(stats.ascent, lessThanOrEqualTo(100));
    });

    test('monotonic descent accumulates descent only', () {
      final series = List<double?>.generate(
          11, (i) => 200.0 - i * 10.0);
      final stats = ElevationStats.fromSamples(series);
      expect(stats.ascent, 0);
      expect(stats.descent, greaterThanOrEqualTo(70));
      expect(stats.descent, lessThanOrEqualTo(100));
    });

    test('sub-noise jitter is dropped', () {
      // ±0.4 m oscillations stay under the 1.0 m noise floor.
      final series = <double?>[];
      for (var i = 0; i < 50; i++) {
        series.add(100.0 + (i.isEven ? 0.4 : -0.4));
      }
      final stats = ElevationStats.fromSamples(series);
      expect(stats.ascent, 0);
      expect(stats.descent, 0);
    });

    test('nulls in the middle of the series are skipped', () {
      final series = <double?>[100, null, 110, null, 120];
      final stats = ElevationStats.fromSamples(series);
      // Three valid samples — 100→110→120 — should still produce ascent.
      expect(stats.ascent, greaterThan(0));
    });

    test('round trip up-and-down records both legs', () {
      final up = List<double?>.generate(11, (i) => 100.0 + i * 10.0);
      final down = List<double?>.generate(11, (i) => 200.0 - i * 10.0);
      final stats = ElevationStats.fromSamples([...up, ...down]);
      expect(stats.ascent, greaterThan(50));
      expect(stats.descent, greaterThan(50));
    });
  });
}
