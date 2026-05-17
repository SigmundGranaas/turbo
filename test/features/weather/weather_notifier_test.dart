import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/weather/api.dart';

class _RecordingFetcher implements WeatherFetcher {
  int calls = 0;
  WeatherForecast Function()? supply;

  @override
  YrAtmosphericService get atmospheric => throw UnimplementedError();

  @override
  YrOceanService get ocean => throw UnimplementedError();

  @override
  Future<WeatherForecast> fetch(
    LatLng position,
    Set<WeatherMetricSource> sources, {
    WeatherForecast? previous,
  }) async {
    calls++;
    return supply!();
  }
}

WeatherForecast _forecast({
  required Duration freshFor,
  List<AtmosphericPoint>? atm,
  List<MarinePoint>? marine,
}) {
  final now = DateTime.now().toUtc();
  return WeatherForecast(
    position: const LatLng(60, 5),
    fetchedAt: now,
    atmosphericExpiresAt: now.add(freshFor),
    marineExpiresAt: null,
    atmosphericLastModified: null,
    marineLastModified: null,
    atmospheric: atm ??
        [
          AtmosphericPoint(
            timeUtc: now,
            airTemperatureC: 10,
            windSpeedMs: 1,
            windFromDeg: null,
            humidity: null,
            pressureHpa: null,
            cloudCoverPercent: null,
            uvIndex: null,
            precipitation1hMm: null,
            symbol1h: null,
            symbol6h: null,
            symbol12h: null,
          )
        ],
    marine: marine ?? const [],
  );
}

void main() {
  group('WeatherForecastNotifier', () {
    test('first read calls the fetcher; second read within fresh window does not',
        () async {
      final fetcher = _RecordingFetcher()
        ..supply = () => _forecast(freshFor: const Duration(minutes: 30));
      final container = ProviderContainer(overrides: [
        weatherFetcherProvider.overrideWith((ref) => fetcher),
      ]);
      addTearDown(container.dispose);

      final req = WeatherRequest(
        position: const LatLng(60, 5),
        sources: const {WeatherMetricSource.atmospheric},
      );

      await container.read(weatherForecastProvider(req).future);
      await container.read(weatherForecastProvider(req).future);
      expect(fetcher.calls, 1);
    });

    test('refresh() forces a new fetch even when cached', () async {
      final fetcher = _RecordingFetcher()
        ..supply = () => _forecast(freshFor: const Duration(minutes: 30));
      final container = ProviderContainer(overrides: [
        weatherFetcherProvider.overrideWith((ref) => fetcher),
      ]);
      addTearDown(container.dispose);

      final req = WeatherRequest(
        position: const LatLng(60, 5),
        sources: const {WeatherMetricSource.atmospheric},
      );
      await container.read(weatherForecastProvider(req).future);
      await container
          .read(weatherForecastProvider(req).notifier)
          .refresh();
      expect(fetcher.calls, 2);
    });

    test('different request shapes do not share cache', () async {
      final fetcher = _RecordingFetcher()
        ..supply = () => _forecast(freshFor: const Duration(minutes: 30));
      final container = ProviderContainer(overrides: [
        weatherFetcherProvider.overrideWith((ref) => fetcher),
      ]);
      addTearDown(container.dispose);

      final atm = WeatherRequest(
        position: const LatLng(60, 5),
        sources: const {WeatherMetricSource.atmospheric},
      );
      final both = WeatherRequest(
        position: const LatLng(60, 5),
        sources: const {
          WeatherMetricSource.atmospheric,
          WeatherMetricSource.marine,
        },
      );
      await container.read(weatherForecastProvider(atm).future);
      await container.read(weatherForecastProvider(both).future);
      expect(fetcher.calls, 2);
    });

    test('WeatherRequest is value-equal by rounded coords + sources', () {
      final a = WeatherRequest(
        position: const LatLng(60.00001, 5.00001),
        sources: const {WeatherMetricSource.atmospheric},
      );
      final b = WeatherRequest(
        position: const LatLng(60.00002, 5.00002),
        sources: const {WeatherMetricSource.atmospheric},
      );
      // Same when rounded to 4 decimals.
      expect(a, equals(b));
      expect(a.hashCode, equals(b.hashCode));

      final c = WeatherRequest(
        position: const LatLng(60.0, 5.0),
        sources: const {
          WeatherMetricSource.atmospheric,
          WeatherMetricSource.marine,
        },
      );
      expect(a, isNot(equals(c)));
    });
  });
}
