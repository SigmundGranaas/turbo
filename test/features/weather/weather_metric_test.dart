import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/weather/api.dart';

void main() {
  group('WeatherMetric', () {
    test('atmospheric metrics carry source=atmospheric', () {
      const atmospheric = {
        WeatherMetric.temperature,
        WeatherMetric.precipitation,
        WeatherMetric.snow,
        WeatherMetric.wind,
        WeatherMetric.humidity,
        WeatherMetric.pressure,
        WeatherMetric.cloudCover,
        WeatherMetric.uvIndex,
      };
      for (final m in atmospheric) {
        expect(m.source, WeatherMetricSource.atmospheric,
            reason: '$m should be atmospheric');
      }
    });

    test('marine metrics carry source=marine', () {
      const marine = {
        WeatherMetric.waveHeight,
        WeatherMetric.waveDirection,
        WeatherMetric.waterTemperature,
      };
      for (final m in marine) {
        expect(m.source, WeatherMetricSource.marine,
            reason: '$m should be marine');
      }
    });

    test('sourcesFor unions the source classifications of opted-in metrics',
        () {
      expect(
        WeatherMetric.sourcesFor(
            const {WeatherMetric.temperature, WeatherMetric.wind}),
        {WeatherMetricSource.atmospheric},
      );
      expect(
        WeatherMetric.sourcesFor(const {WeatherMetric.waveHeight}),
        {WeatherMetricSource.marine},
      );
      expect(
        WeatherMetric.sourcesFor(const {
          WeatherMetric.temperature,
          WeatherMetric.waterTemperature,
        }),
        {WeatherMetricSource.atmospheric, WeatherMetricSource.marine},
      );
    });

    test('every metric has a stable code that round-trips by code lookup', () {
      for (final m in WeatherMetric.values) {
        expect(WeatherMetric.byCode(m.code), m,
            reason: 'code ${m.code} should resolve back to $m');
      }
    });

    test('byCode returns null for an unknown code', () {
      expect(WeatherMetric.byCode('nonsense'), isNull);
    });
  });
}
