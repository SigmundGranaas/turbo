import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/markers/api.dart';
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
  YrSunriseService get sunrise => throw UnimplementedError();

  @override
  MetAlertsService get alerts => throw UnimplementedError();

  @override
  Future<WeatherForecast> fetch(
    LatLng position, {
    WeatherForecast? previous,
  }) async {
    if (error != null) throw error!;
    return supply!();
  }
}

Marker _mkMarker() => Marker(
      uuid: 'marker-1',
      title: 'Test',
      position: const LatLng(60.39, 5.32),
    );

AtmosphericPoint _atmAt(DateTime t, double temp,
        {double wind = 3, WeatherSymbol? symbol}) =>
    AtmosphericPoint(
      timeUtc: t,
      airTemperatureC: temp,
      windSpeedMs: wind,
      windFromDeg: 180,
      humidity: null,
      pressureHpa: null,
      cloudCoverPercent: null,
      uvIndex: null,
      precipitation1hMm: null,
      symbol1h: symbol,
      symbol6h: null,
      symbol12h: null,
    );

WeatherForecast _forecast({required List<AtmosphericPoint> atm}) {
  final now = DateTime.now().toUtc();
  return WeatherForecast(
    position: const LatLng(60.39, 5.32),
    fetchedAt: now,
    atmosphericExpiresAt: now.add(const Duration(minutes: 30)),
    marineExpiresAt: null,
    atmosphericLastModified: null,
    marineLastModified: null,
    atmospheric: atm,
    marine: const [],
  );
}

Future<void> _pump(
  WidgetTester tester, {
  required WeatherForecast Function() supply,
  Object? error,
}) async {
  final fetcher = _StubFetcher(supply);
  if (error != null) fetcher.error = error;
  await pumpTestApp(
    tester,
    WeatherSummaryRow(marker: _mkMarker()),
    overrides: [
      weatherFetcherProvider.overrideWith((ref) => fetcher),
    ],
    settle: false,
  );
  await tester.pumpAndSettle();
}

void main() {
  group('WeatherSummaryRow', () {
    testWidgets(
        'data state: symbol on left, temp, wind arrow + speed on right, chevron',
        (tester) async {
      final base = DateTime.now().toUtc();
      await _pump(
        tester,
        supply: () => _forecast(
          atm: [_atmAt(base, 14, symbol: WeatherSymbol.fromCode('clearsky_day'))],
        ),
      );
      expect(find.byKey(const Key('weather-summary-row')), findsOneWidget);
      expect(find.text('14°'), findsOneWidget);
      // Right-aligned wind: arrow icon + speed text.
      expect(find.byIcon(Icons.navigation), findsOneWidget);
      expect(find.text('3.0 m/s'), findsOneWidget);
      // No compass cardinal text anymore.
      expect(find.textContaining('m/s S'), findsNothing);
      expect(find.byIcon(Icons.chevron_right), findsOneWidget);
      expect(find.byKey(const Key('weather-symbol-clearsky_day')),
          findsOneWidget);
    });

    testWidgets('tapping the row opens the detail bottom sheet',
        (tester) async {
      final base = DateTime.now().toUtc();
      await _pump(
        tester,
        supply: () => _forecast(
          atm: [
            for (var i = 0; i < 5; i++)
              _atmAt(base.add(Duration(hours: i)), 10),
          ],
        ),
      );
      await tester.tap(find.byKey(const Key('weather-summary-row')));
      await tester.pumpAndSettle();

      // Sheet's day strip + preset tabs render.
      expect(find.byKey(const Key('weather-detail-day-strip')), findsOneWidget);
      expect(find.byKey(const Key('weather-detail-preset-tabs')),
          findsOneWidget);
    });

    testWidgets('error state shows retry; tapping triggers refresh',
        (tester) async {
      await _pump(
        tester,
        supply: () => _forecast(atm: const []),
        error: const YrServiceException(503, 'fail'),
      );
      expect(find.byKey(const Key('weather-summary-error')), findsOneWidget);
      expect(find.text("Couldn't load weather"), findsOneWidget);
      expect(find.text('Retry'), findsOneWidget);
    });

    testWidgets('hides itself when forecast has no atmospheric data',
        (tester) async {
      await _pump(tester, supply: () => _forecast(atm: const []));
      expect(find.byKey(const Key('weather-summary-row')), findsNothing);
    });
  });
}
