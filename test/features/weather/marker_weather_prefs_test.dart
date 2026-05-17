import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/weather/api.dart';

void main() {
  group('MarkerWeatherPrefs', () {
    test('defaults() yields {temperature, wind, precipitation} for any uuid',
        () {
      final p = MarkerWeatherPrefs.defaults('abc');
      expect(p.markerUuid, 'abc');
      expect(
        p.metrics,
        {
          WeatherMetric.temperature,
          WeatherMetric.wind,
          WeatherMetric.precipitation,
        },
      );
    });

    test('metrics set is unmodifiable', () {
      final p = MarkerWeatherPrefs.defaults('abc');
      expect(() => p.metrics.add(WeatherMetric.snow),
          throwsUnsupportedError);
    });

    test('JSON round-trip preserves the metric set and marker uuid', () {
      final original = MarkerWeatherPrefs(
        markerUuid: 'aaa-111',
        metrics: const {
          WeatherMetric.temperature,
          WeatherMetric.waveHeight,
          WeatherMetric.snow,
        },
      );
      final encoded = jsonEncode(original.toJson());
      final decoded = MarkerWeatherPrefs.fromJson(
          'aaa-111', jsonDecode(encoded) as Map<String, dynamic>);
      expect(decoded.markerUuid, 'aaa-111');
      expect(decoded.metrics, original.metrics);
    });

    test('fromJson tolerates unknown metric codes by dropping them', () {
      final json = {
        'metrics': ['temperature', 'bogus_metric', 'waveHeight'],
      };
      final p = MarkerWeatherPrefs.fromJson('uid', json);
      expect(p.metrics, {WeatherMetric.temperature, WeatherMetric.waveHeight});
    });

    test('copyWith replaces metrics but keeps uuid', () {
      final p = MarkerWeatherPrefs.defaults('abc');
      final updated = p.copyWith(metrics: {WeatherMetric.snow});
      expect(updated.markerUuid, 'abc');
      expect(updated.metrics, {WeatherMetric.snow});
    });

    test('equality is value-based on uuid + metrics', () {
      final a = MarkerWeatherPrefs(
          markerUuid: 'x',
          metrics: const {WeatherMetric.temperature, WeatherMetric.wind});
      final b = MarkerWeatherPrefs(
          markerUuid: 'x',
          metrics: const {WeatherMetric.wind, WeatherMetric.temperature});
      final c = MarkerWeatherPrefs(
          markerUuid: 'y',
          metrics: const {WeatherMetric.temperature, WeatherMetric.wind});
      expect(a, equals(b));
      expect(a, isNot(equals(c)));
      expect(a.hashCode, equals(b.hashCode));
    });
  });
}
