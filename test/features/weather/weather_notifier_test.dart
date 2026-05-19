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
  YrSunriseService get sunrise => throw UnimplementedError();

  @override
  MetAlertsService get alerts => throw UnimplementedError();

  @override
  Future<WeatherForecast> fetch(
    LatLng position, {
    WeatherForecast? previous,
  }) async {
    calls++;
    return supply!();
  }
}

WeatherForecast _forecast({required Duration freshFor}) {
  final now = DateTime.now().toUtc();
  return WeatherForecast(
    position: const LatLng(60, 5),
    fetchedAt: now,
    atmosphericExpiresAt: now.add(freshFor),
    marineExpiresAt: null,
    atmosphericLastModified: null,
    marineLastModified: null,
    atmospheric: [
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
    marine: const [],
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

      const pos = LatLng(60, 5);
      await container.read(weatherForecastProvider(pos).future);
      await container.read(weatherForecastProvider(pos).future);
      expect(fetcher.calls, 1);
    });

    test('refresh() forces a new fetch even when cached', () async {
      final fetcher = _RecordingFetcher()
        ..supply = () => _forecast(freshFor: const Duration(minutes: 30));
      final container = ProviderContainer(overrides: [
        weatherFetcherProvider.overrideWith((ref) => fetcher),
      ]);
      addTearDown(container.dispose);

      const pos = LatLng(60, 5);
      await container.read(weatherForecastProvider(pos).future);
      await container.read(weatherForecastProvider(pos).notifier).refresh();
      expect(fetcher.calls, 2);
    });

    test('different positions do not share cache', () async {
      final fetcher = _RecordingFetcher()
        ..supply = () => _forecast(freshFor: const Duration(minutes: 30));
      final container = ProviderContainer(overrides: [
        weatherFetcherProvider.overrideWith((ref) => fetcher),
      ]);
      addTearDown(container.dispose);

      await container
          .read(weatherForecastProvider(const LatLng(60, 5)).future);
      await container
          .read(weatherForecastProvider(const LatLng(70, 10)).future);
      expect(fetcher.calls, 2);
    });

  });
}
