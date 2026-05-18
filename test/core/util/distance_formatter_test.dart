import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/util/distance_formatter.dart';

void main() {
  group('formatDistance(metric)', () {
    test('uses m below 1 km, rounded to the nearest meter', () {
      expect(formatDistance(0, DistanceUnit.metric), '0 m');
      expect(formatDistance(1, DistanceUnit.metric), '1 m');
      expect(formatDistance(123.4, DistanceUnit.metric), '123 m');
      expect(formatDistance(999.4, DistanceUnit.metric), '999 m');
    });

    test('switches to km at 1000 m, formatted with 2 decimals', () {
      expect(formatDistance(1000, DistanceUnit.metric), '1.00 km');
      expect(formatDistance(1234.5, DistanceUnit.metric), '1.23 km');
      expect(formatDistance(125000, DistanceUnit.metric), '125.00 km');
    });
  });

  group('formatDistance(imperial)', () {
    test('uses ft below 1000 ft, rounded to the nearest foot', () {
      expect(formatDistance(0, DistanceUnit.imperial), '0 ft');
      // 100 m ≈ 328 ft
      expect(formatDistance(100, DistanceUnit.imperial), '328 ft');
      // 304.8 m = exactly 1000 ft → boundary still in ft
      expect(formatDistance(304.79, DistanceUnit.imperial), '1000 ft');
    });

    test('switches to mi at 1000 ft, formatted with 2 decimals', () {
      // 304.8 m = exactly 1000 ft → boundary crosses to mi (feet >= 1000)
      expect(formatDistance(304.8, DistanceUnit.imperial), '0.19 mi');
      // 1 mi = 1609.344 m
      expect(formatDistance(1609.344, DistanceUnit.imperial), '1.00 mi');
      expect(formatDistance(16093.44, DistanceUnit.imperial), '10.00 mi');
    });
  });

  group('formatDistance(nautical)', () {
    test('uses m below 1 NM (1852 m), rounded to the nearest meter', () {
      expect(formatDistance(0, DistanceUnit.nautical), '0 m');
      expect(formatDistance(500, DistanceUnit.nautical), '500 m');
      expect(formatDistance(1851.4, DistanceUnit.nautical), '1851 m');
    });

    test('switches to NM at 1 NM, formatted with 2 decimals', () {
      expect(formatDistance(1852, DistanceUnit.nautical), '1.00 NM');
      expect(formatDistance(3704, DistanceUnit.nautical), '2.00 NM');
      // 10 NM
      expect(formatDistance(18520, DistanceUnit.nautical), '10.00 NM');
    });
  });

  group('formatDistance(edge cases)', () {
    test('NaN and infinity render as a placeholder', () {
      expect(formatDistance(double.nan, DistanceUnit.metric), '—');
      expect(formatDistance(double.infinity, DistanceUnit.imperial), '—');
      expect(formatDistance(double.nan, DistanceUnit.nautical), '—');
      expect(
          formatDistance(double.negativeInfinity, DistanceUnit.metric), '—');
    });
  });

  group('formatSpeed', () {
    test('metric renders km/h with one decimal', () {
      expect(formatSpeed(0, DistanceUnit.metric), '0.0 km/h');
      // 10 m/s = 36.0 km/h
      expect(formatSpeed(10, DistanceUnit.metric), '36.0 km/h');
    });

    test('imperial renders mph with one decimal', () {
      // 10 m/s ≈ 22.4 mph
      expect(formatSpeed(10, DistanceUnit.imperial), '22.4 mph');
    });

    test('nautical renders knots with one decimal', () {
      // 1 knot = 0.514444 m/s, so 10 m/s ≈ 19.4 kn
      expect(formatSpeed(10, DistanceUnit.nautical), '19.4 kn');
      // 1 m/s ≈ 1.9 kn
      expect(formatSpeed(1, DistanceUnit.nautical), '1.9 kn');
    });

    test('NaN/infinity render as a placeholder', () {
      expect(formatSpeed(double.nan, DistanceUnit.nautical), '—');
      expect(formatSpeed(double.infinity, DistanceUnit.metric), '—');
    });

    test('negative speeds are clamped to zero', () {
      expect(formatSpeed(-5, DistanceUnit.nautical), '0.0 kn');
    });
  });

  group('DistanceUnit.fromName', () {
    test('round-trips the canonical names', () {
      expect(DistanceUnit.fromName('metric'), DistanceUnit.metric);
      expect(DistanceUnit.fromName('imperial'), DistanceUnit.imperial);
      expect(DistanceUnit.fromName('nautical'), DistanceUnit.nautical);
    });

    test('unknown or null name falls back to metric', () {
      expect(DistanceUnit.fromName(null), DistanceUnit.metric);
      expect(DistanceUnit.fromName(''), DistanceUnit.metric);
      expect(DistanceUnit.fromName('km'), DistanceUnit.metric);
    });
  });
}
