import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/weather/api.dart';

import '../../helpers/pump_app.dart';

class _StubFetcher implements WeatherFetcher {
  _StubFetcher(this.supply);
  int calls = 0;
  WeatherForecast Function()? supply;
  Object? error;

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
    if (error != null) throw error!;
    return supply!();
  }
}

Marker _mkMarker() => Marker(
      uuid: 'marker-1',
      title: 'Test',
      position: const LatLng(60.39, 5.32),
    );

WeatherForecast _forecast({
  required List<AtmosphericPoint> atm,
  List<MarinePoint> marine = const [],
  DateTime? expires,
}) {
  final now = DateTime.now().toUtc();
  return WeatherForecast(
    position: const LatLng(60.39, 5.32),
    fetchedAt: now,
    atmosphericExpiresAt: expires ?? now.add(const Duration(minutes: 30)),
    marineExpiresAt: marine.isEmpty ? null : now.add(const Duration(minutes: 30)),
    atmosphericLastModified: null,
    marineLastModified: null,
    atmospheric: atm,
    marine: marine,
  );
}

AtmosphericPoint _atmAt(DateTime t, double temp,
        {double wind = 1, double? precip}) =>
    AtmosphericPoint(
      timeUtc: t,
      airTemperatureC: temp,
      windSpeedMs: wind,
      windFromDeg: 180,
      humidity: null,
      pressureHpa: null,
      cloudCoverPercent: null,
      uvIndex: null,
      precipitation1hMm: precip,
      symbol1h: null,
      symbol6h: null,
      symbol12h: null,
    );

class _NoopPrefsStore implements MarkerWeatherPrefsStore {
  @override
  Future<MarkerWeatherPrefs?> get(String _) async => null;
  @override
  Future<void> upsert(MarkerWeatherPrefs _) async {}
  @override
  Future<void> delete(String _) async {}
}

Future<void> _pump(WidgetTester tester, {
  required WeatherForecast Function() supply,
  Set<WeatherMetric>? metrics,
  Object? error,
}) async {
  final fetcher = _StubFetcher(supply);
  if (error != null) fetcher.error = error;
  await pumpTestApp(
    tester,
    WeatherSection(marker: _mkMarker()),
    overrides: [
      markerWeatherPrefsStoreProvider
          .overrideWith((ref) async => _NoopPrefsStore()),
      weatherFetcherProvider.overrideWith((ref) => fetcher),
      if (metrics != null)
        markerWeatherPrefsProvider('marker-1').overrideWith(
          () => _StubPrefsNotifier(metrics),
        ),
    ],
    settle: false,
  );
  await tester.pumpAndSettle();
}

class _StubPrefsNotifier extends MarkerWeatherPrefsNotifier {
  _StubPrefsNotifier(this.initial) : super('marker-1');
  final Set<WeatherMetric> initial;

  @override
  MarkerWeatherPrefs build() {
    // Skip the parent's _load() call so the test doesn't need to override the
    // store provider. The initial set IS the state for the test.
    return MarkerWeatherPrefs(markerUuid: 'marker-1', metrics: initial);
  }

  @override
  Future<void> setMetrics(Set<WeatherMetric> metrics) async {
    // Test-only no-op write to avoid hitting the (unoverridden) store.
    state = state.copyWith(metrics: metrics);
  }
}

void main() {
  group('WeatherSection', () {
    testWidgets('renders default metric rows from a successful forecast',
        (tester) async {
      await _pump(
        tester,
        supply: () => _forecast(
          atm: [_atmAt(DateTime.now().toUtc(), 14.2, wind: 3.1, precip: 0.4)],
        ),
      );
      // Header + attribution always present in the data state.
      expect(find.text('Forecast'), findsOneWidget);
      expect(find.text('Weather data from MET Norway / yr.no'), findsOneWidget);
      // Default opted-in rows: Temperature, Wind, Precipitation.
      expect(find.text('Temperature'), findsOneWidget);
      expect(find.text('Wind'), findsOneWidget);
      expect(find.text('Precipitation'), findsOneWidget);
      // Snow/UV/etc are not in the default set.
      expect(find.text('Snow'), findsNothing);
      expect(find.text('UV index'), findsNothing);
    });

    testWidgets('hides marine rows when hasMarineData is false',
        (tester) async {
      await _pump(
        tester,
        supply: () => _forecast(
          atm: [_atmAt(DateTime.now().toUtc(), 14.2)],
        ),
        metrics: {WeatherMetric.temperature, WeatherMetric.waveHeight},
      );
      expect(find.text('Temperature'), findsOneWidget);
      // Wave height row is hidden when there's no marine data.
      expect(find.text('Wave height'), findsNothing);
    });

    testWidgets('renders marine rows when marine data is present',
        (tester) async {
      await _pump(
        tester,
        supply: () => _forecast(
          atm: [_atmAt(DateTime.now().toUtc(), 14.2)],
          marine: [
            MarinePoint(
              timeUtc: DateTime.now().toUtc(),
              waveHeightM: 1.2,
              waveFromDeg: 280,
              seaWaterTemperatureC: 11.3,
              seaWaterSpeedMs: 0.2,
            )
          ],
        ),
        metrics: {WeatherMetric.temperature, WeatherMetric.waveHeight},
      );
      expect(find.text('Temperature'), findsOneWidget);
      expect(find.text('Wave height'), findsOneWidget);
    });

    testWidgets('error state shows retry button', (tester) async {
      await _pump(
        tester,
        supply: () => _forecast(atm: [_atmAt(DateTime.now().toUtc(), 14)]),
        error: const YrServiceException(503, 'fail'),
      );
      expect(find.text("Couldn't load weather"), findsOneWidget);
      expect(find.text('Retry'), findsOneWidget);
    });
  });
}
