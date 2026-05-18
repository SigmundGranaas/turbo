import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/weather/api.dart';

import '../../helpers/pump_app.dart';

class _StubFetcher implements WeatherFetcher {
  _StubFetcher(this.supply);
  WeatherForecast Function()? supply;

  @override
  YrAtmosphericService get atmospheric => throw UnimplementedError();
  @override
  YrOceanService get ocean => throw UnimplementedError();

  @override
  Future<WeatherForecast> fetch(
    LatLng position, {
    WeatherForecast? previous,
  }) async {
    return supply!();
  }
}

Marker _mkMarker() => Marker(
      uuid: 'marker-1',
      title: 'Test Marker',
      position: const LatLng(60.39, 5.32),
    );

AtmosphericPoint _atmAt(DateTime t, double temp, {WeatherSymbol? symbol}) =>
    AtmosphericPoint(
      timeUtc: t,
      airTemperatureC: temp,
      windSpeedMs: 3,
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
}) async {
  tester.view.physicalSize = const Size(1200, 2000);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(() {
    tester.view.resetPhysicalSize();
    tester.view.resetDevicePixelRatio();
  });
  await pumpTestApp(
    tester,
    WeatherDetailPage(marker: _mkMarker()),
    overrides: [
      weatherFetcherProvider.overrideWith((ref) => _StubFetcher(supply)),
    ],
    settle: false,
  );
  await tester.pumpAndSettle();
}

void main() {
  group('WeatherDetailPage', () {
    testWidgets('renders nowcast, hourly, daily, attribution', (tester) async {
      final base = DateTime.now().toUtc();
      await _pump(
        tester,
        supply: () => _forecast(
          atm: [
            for (var i = 0; i < 30; i++)
              _atmAt(base.add(Duration(hours: i)), 10 + (i % 5).toDouble()),
          ],
        ),
      );
      expect(find.text('Test Marker'), findsOneWidget);
      expect(find.byKey(const Key('weather-detail-nowcast')), findsOneWidget);
      expect(find.byKey(const Key('weather-detail-hourly')), findsOneWidget);
      expect(find.byKey(const Key('weather-detail-daily')), findsOneWidget);
      expect(find.text('Next 24 hours'), findsOneWidget);
      expect(find.text('Next 9 days'), findsOneWidget);
      expect(find.text('Weather data from MET Norway / yr.no'), findsOneWidget);
    });

    testWidgets('marine block appears when forecast carries marine data',
        (tester) async {
      final base = DateTime.now().toUtc();
      await _pump(
        tester,
        supply: () => _forecast(
          atm: [_atmAt(base, 10)],
          marine: [
            MarinePoint(
              timeUtc: base,
              waveHeightM: 1.5,
              waveFromDeg: 270,
              seaWaterTemperatureC: 12.0,
              seaWaterSpeedMs: 0.3,
            ),
          ],
        ),
      );
      expect(find.byKey(const Key('weather-detail-marine')), findsOneWidget);
      expect(find.text('Sea conditions'), findsOneWidget);
      expect(find.textContaining('1.5 m'), findsOneWidget);
      expect(find.textContaining('12°C'), findsOneWidget);
    });

    testWidgets('marine block hidden when forecast has no marine data',
        (tester) async {
      final base = DateTime.now().toUtc();
      await _pump(
        tester,
        supply: () => _forecast(atm: [_atmAt(base, 10)]),
      );
      expect(find.byKey(const Key('weather-detail-marine')), findsNothing);
    });
  });
}
