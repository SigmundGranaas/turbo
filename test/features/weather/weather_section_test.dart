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
        {double wind = 1, double? precip, WeatherSymbol? symbol}) =>
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
      symbol1h: symbol,
      symbol6h: null,
      symbol12h: null,
    );

WeatherForecast _forecast({
  required List<AtmosphericPoint> atm,
  List<MarinePoint> marine = const [],
}) {
  final now = DateTime.now().toUtc();
  return WeatherForecast(
    position: const LatLng(60.39, 5.32),
    fetchedAt: now,
    atmosphericExpiresAt: now.add(const Duration(minutes: 30)),
    marineExpiresAt:
        marine.isEmpty ? null : now.add(const Duration(minutes: 30)),
    atmosphericLastModified: null,
    marineLastModified: null,
    atmospheric: atm,
    marine: marine,
  );
}

Future<void> _pump(
  WidgetTester tester, {
  required WeatherForecast Function() supply,
  Object? error,
}) async {
  // Wide viewport so the hourly + daily strips have room.
  tester.view.physicalSize = const Size(1200, 1600);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(() {
    tester.view.resetPhysicalSize();
    tester.view.resetDevicePixelRatio();
  });
  final fetcher = _StubFetcher(supply);
  if (error != null) fetcher.error = error;
  await pumpTestApp(
    tester,
    SingleChildScrollView(child: WeatherSection(marker: _mkMarker())),
    overrides: [
      weatherFetcherProvider.overrideWith((ref) => fetcher),
    ],
    settle: false,
  );
  await tester.pumpAndSettle();
}

void main() {
  group('WeatherSection', () {
    testWidgets('renders header, now-cast, hourly + daily strips, attribution',
        (tester) async {
      // 26 atmospheric points: one current + 25 future hours.
      final base = DateTime.now().toUtc();
      final atm = [
        for (var i = 0; i < 26; i++)
          _atmAt(base.add(Duration(hours: i)), 10 + (i % 5).toDouble()),
      ];
      await _pump(tester, supply: () => _forecast(atm: atm));

      // Header + attribution.
      expect(find.text('Forecast'), findsOneWidget);
      expect(find.text('Weather data from MET Norway / yr.no'), findsOneWidget);
      // Now-cast temperature (largest text). The current point has temp 10.
      expect(find.text('10°'), findsWidgets);
      // Both strips are present.
      expect(find.byKey(const Key('weather-hourly-strip')), findsOneWidget);
      expect(find.byKey(const Key('weather-daily-strip')), findsOneWidget);
    });

    testWidgets('marine block hidden when forecast has no marine data',
        (tester) async {
      final base = DateTime.now().toUtc();
      await _pump(
        tester,
        supply: () => _forecast(atm: [_atmAt(base, 10)]),
      );
      expect(find.byKey(const Key('weather-marine-block')), findsNothing);
    });

    testWidgets('marine block shows when forecast has marine data',
        (tester) async {
      final base = DateTime.now().toUtc();
      await _pump(
        tester,
        supply: () => _forecast(
          atm: [_atmAt(base, 10)],
          marine: [
            MarinePoint(
              timeUtc: base,
              waveHeightM: 1.2,
              waveFromDeg: 280,
              seaWaterTemperatureC: 11.3,
              seaWaterSpeedMs: 0.2,
            ),
          ],
        ),
      );
      expect(find.byKey(const Key('weather-marine-block')), findsOneWidget);
      expect(find.text('Sea conditions'), findsOneWidget);
      expect(find.textContaining('1.2 m'), findsOneWidget);
      expect(find.textContaining('11°C'), findsOneWidget);
    });

    testWidgets('error state shows retry button', (tester) async {
      await _pump(
        tester,
        supply: () => _forecast(atm: const []),
        error: const YrServiceException(503, 'fail'),
      );
      expect(find.text("Couldn't load weather"), findsOneWidget);
      expect(find.text('Retry'), findsOneWidget);
    });

    testWidgets('renders MET SVG for known symbol codes', (tester) async {
      final base = DateTime.now().toUtc();
      await _pump(
        tester,
        supply: () => _forecast(
          atm: [_atmAt(base, 10, symbol: WeatherSymbol.fromCode('clearsky_day'))],
        ),
      );
      // The symbol appears in the now-cast, the hourly strip's first cell,
      // and the daily strip's midday cell — same key, three sites.
      expect(find.byKey(const Key('weather-symbol-clearsky_day')),
          findsWidgets);
    });
  });
}
