import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/weather/api.dart';

import '../../helpers/pump_app.dart';

class _StubFetcher implements WeatherFetcher {
  _StubFetcher(this.supply);
  WeatherForecast Function()? supply;
  Object? error;

  @override
  YrAtmosphericService get atmospheric => throw UnimplementedError();

  @override
  YrOceanService get ocean => throw UnimplementedError();

  @override
  Future<WeatherForecast> fetch(
    LatLng position, {
    WeatherForecast? previous,
  }) async {
    if (error != null) throw error!;
    return supply!();
  }
}

class _SeededPosition extends LastPositionNotifier {
  final PositionSnapshot? seed;
  _SeededPosition(this.seed);
  @override
  PositionSnapshot? build() => seed;
}

AtmosphericPoint _atm(
  DateTime t, {
  double wind = 4,
  double? gust,
  double from = 180,
}) =>
    AtmosphericPoint(
      timeUtc: t,
      airTemperatureC: 10,
      windSpeedMs: wind,
      windGustMs: gust,
      windFromDeg: from,
      humidity: null,
      pressureHpa: null,
      cloudCoverPercent: null,
      uvIndex: null,
      precipitation1hMm: null,
      symbol1h: null,
      symbol6h: null,
      symbol12h: null,
    );

WeatherForecast _forecast(List<AtmosphericPoint> points) {
  final now = DateTime.now().toUtc();
  return WeatherForecast(
    position: const LatLng(60.39, 5.32),
    fetchedAt: now,
    atmosphericExpiresAt: now.add(const Duration(minutes: 30)),
    marineExpiresAt: null,
    atmosphericLastModified: null,
    marineLastModified: null,
    atmospheric: points,
    marine: const [],
  );
}

Future<void> _pump(
  WidgetTester tester, {
  required bool stripEnabled,
  PositionSnapshot? position,
  WeatherForecast Function()? supply,
}) async {
  SharedPreferences.setMockInitialValues({'showWindStrip': stripEnabled});
  final fetcher = supply == null ? null : _StubFetcher(supply);
  await pumpTestApp(
    tester,
    const MarineWindStrip(),
    resetSharedPrefs: false,
    overrides: [
      lastPositionProvider.overrideWith(() => _SeededPosition(position)),
      if (fetcher != null)
        weatherFetcherProvider.overrideWith((ref) => fetcher),
    ],
    settle: false,
  );
  await tester.pumpAndSettle();
}

void main() {
  group('MarineWindStrip', () {
    testWidgets('renders nothing when the setting is off', (tester) async {
      await _pump(
        tester,
        stripEnabled: false,
        position: PositionSnapshot(latLng: const LatLng(60, 5)),
        supply: () => _forecast([_atm(DateTime.now().toUtc())]),
      );
      expect(find.textContaining('m/s'), findsNothing);
      expect(find.byIcon(Icons.air), findsNothing);
    });

    testWidgets('shows a placeholder when there is no GPS fix yet',
        (tester) async {
      await _pump(
        tester,
        stripEnabled: true,
        position: null,
        // supply still required to keep providers wired, but the strip never
        // reaches the fetch because lastPositionProvider is null.
        supply: () => _forecast(const []),
      );
      expect(find.text('No wind data for this location'), findsOneWidget);
      expect(find.byIcon(Icons.air), findsOneWidget);
    });

    testWidgets(
        'renders current wind, gust readout, and six trend bars from MET data',
        (tester) async {
      final base = DateTime.now().toUtc();
      await _pump(
        tester,
        stripEnabled: true,
        position: PositionSnapshot(latLng: const LatLng(60.39, 5.32)),
        supply: () => _forecast([
          _atm(base, wind: 6.5, gust: 11.2, from: 320),
          _atm(base.add(const Duration(hours: 1)), wind: 7.0),
          _atm(base.add(const Duration(hours: 2)), wind: 8.5),
          _atm(base.add(const Duration(hours: 3)), wind: 9.0),
          _atm(base.add(const Duration(hours: 4)), wind: 9.5),
          _atm(base.add(const Duration(hours: 5)), wind: 10.0),
          _atm(base.add(const Duration(hours: 6)), wind: 8.0),
        ]),
      );

      // Current wind speed in m/s (Norwegian convention; users are used to it).
      expect(find.text('6.5 m/s'), findsOneWidget);
      // Gust readout with localized "gust" label.
      expect(find.textContaining('gust 11.2'), findsOneWidget);
      // Wind direction arrow is rendered.
      expect(find.byIcon(Icons.navigation), findsOneWidget);
    });

    testWidgets('shows placeholder when MET returns no atmospheric points',
        (tester) async {
      await _pump(
        tester,
        stripEnabled: true,
        position: PositionSnapshot(latLng: const LatLng(60.39, 5.32)),
        supply: () => _forecast(const []),
      );
      expect(find.text('No wind data for this location'), findsOneWidget);
    });

    testWidgets('shows placeholder when the upstream fetch fails',
        (tester) async {
      SharedPreferences.setMockInitialValues({'showWindStrip': true});
      final fetcher = _StubFetcher(() => _forecast(const []));
      fetcher.error = const YrServiceException(503, 'down');

      await pumpTestApp(
        tester,
        const MarineWindStrip(),
        resetSharedPrefs: false,
        overrides: [
          lastPositionProvider.overrideWith(
              () => _SeededPosition(PositionSnapshot(latLng: const LatLng(60, 5)))),
          weatherFetcherProvider.overrideWith((ref) => fetcher),
        ],
        settle: false,
      );
      await tester.pumpAndSettle();
      expect(find.text('No wind data for this location'), findsOneWidget);
    });
  });
}
