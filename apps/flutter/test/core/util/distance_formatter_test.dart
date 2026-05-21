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

  group('formatDistance(edge cases)', () {
    test('NaN and infinity render as a placeholder', () {
      expect(formatDistance(double.nan, DistanceUnit.metric), '—');
      expect(formatDistance(double.infinity, DistanceUnit.imperial), '—');
      expect(
          formatDistance(double.negativeInfinity, DistanceUnit.metric), '—');
    });
  });

  group('DistanceUnit.fromName', () {
    test('round-trips the canonical names', () {
      expect(DistanceUnit.fromName('metric'), DistanceUnit.metric);
      expect(DistanceUnit.fromName('imperial'), DistanceUnit.imperial);
    });

    test('unknown or null name falls back to metric', () {
      expect(DistanceUnit.fromName(null), DistanceUnit.metric);
      expect(DistanceUnit.fromName(''), DistanceUnit.metric);
      expect(DistanceUnit.fromName('km'), DistanceUnit.metric);
    });
  });
}
