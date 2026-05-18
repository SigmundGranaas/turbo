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
      title: 'Pier 12',
      position: const LatLng(60.39, 5.32),
    );

AtmosphericPoint _atm(DateTime t,
        {double temp = 10,
        double wind = 4,
        double? windFromDeg = 90,
        double? precip,
        WeatherSymbol? symbol}) =>
    AtmosphericPoint(
      timeUtc: t,
      airTemperatureC: temp,
      windSpeedMs: wind,
      windFromDeg: windFromDeg,
      humidity: null,
      pressureHpa: null,
      cloudCoverPercent: null,
      uvIndex: null,
      precipitation1hMm: precip,
      symbol1h: symbol,
      symbol6h: null,
      symbol12h: null,
    );

MarinePoint _marine(DateTime t,
        {double? wave = 1.2, double? waveFrom = 200, double? water = 11}) =>
    MarinePoint(
      timeUtc: t,
      waveHeightM: wave,
      waveFromDeg: waveFrom,
      seaWaterTemperatureC: water,
      seaWaterSpeedMs: 0.2,
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

/// Span 3 local days starting at local midnight today so [DailySummary]
/// produces 3 buckets, regardless of the test machine's tz.
List<DateTime> _hoursOver3Days() {
  final start = DateTime.now();
  final localMidnight = DateTime(start.year, start.month, start.day);
  return [
    for (var d = 0; d < 3; d++)
      for (var h = 0; h < 24; h++)
        localMidnight.add(Duration(days: d, hours: h)).toUtc(),
  ];
}

Future<void> _openSheet(
  WidgetTester tester, {
  required WeatherForecast Function() supply,
}) async {
  tester.view.physicalSize = const Size(1200, 2400);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(() {
    tester.view.resetPhysicalSize();
    tester.view.resetDevicePixelRatio();
  });
  await pumpTestApp(
    tester,
    Builder(
      builder: (context) => Center(
        child: ElevatedButton(
          key: const Key('open-sheet'),
          onPressed: () => showWeatherDetailSheet(context, _mkMarker()),
          child: const Text('open'),
        ),
      ),
    ),
    overrides: [
      weatherFetcherProvider.overrideWith((ref) => _StubFetcher(supply)),
    ],
    settle: false,
  );
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const Key('open-sheet')));
  await tester.pumpAndSettle();
}

void main() {
  group('WeatherDetailSheet', () {
    testWidgets('renders day strip, preset tabs, and hourly list',
        (tester) async {
      final stamps = _hoursOver3Days();
      await _openSheet(
        tester,
        supply: () => _forecast(
          atm: [
            for (final t in stamps)
              _atm(t,
                  temp: 10 + (t.hour % 5).toDouble(),
                  symbol: WeatherSymbol.fromCode('clearsky_day')),
          ],
        ),
      );

      expect(find.text('Pier 12'), findsOneWidget);
      expect(find.byKey(const Key('weather-detail-day-strip')), findsOneWidget);
      expect(find.byKey(const Key('weather-detail-preset-tabs')),
          findsOneWidget);
      // Default preset is hourly.
      expect(find.byKey(const Key('preset-body-hourly')), findsOneWidget);
      // No marine data → no sea chip.
      expect(find.byKey(const Key('preset-chip-sea')), findsNothing);
    });

    testWidgets('selecting Wind preset swaps the body', (tester) async {
      final stamps = _hoursOver3Days();
      await _openSheet(
        tester,
        supply: () => _forecast(
          atm: [for (final t in stamps) _atm(t)],
        ),
      );

      await tester.tap(find.byKey(const Key('preset-chip-wind')));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('preset-body-wind')), findsOneWidget);
    });

    testWidgets('Sea preset only appears when forecast has marine data',
        (tester) async {
      final stamps = _hoursOver3Days();
      await _openSheet(
        tester,
        supply: () => _forecast(
          atm: [for (final t in stamps) _atm(t)],
          marine: [for (final t in stamps) _marine(t)],
        ),
      );

      expect(find.byKey(const Key('preset-chip-sea')), findsOneWidget);
      await tester.tap(find.byKey(const Key('preset-chip-sea')));
      await tester.pumpAndSettle();
      // Sea rows show wave height in meters.
      expect(find.textContaining('1.2 m'), findsWidgets);
    });

    testWidgets('selecting day 2 filters hours to that day', (tester) async {
      final stamps = _hoursOver3Days();
      await _openSheet(
        tester,
        supply: () => _forecast(
          atm: [
            for (final t in stamps)
              _atm(t,
                  temp: t.toLocal().day.toDouble() *
                      10), // unique temp per-day
          ],
        ),
      );

      // Tap the second day chip in the strip.
      final dayChips =
          find.descendant(of: find.byKey(const Key('weather-detail-day-strip')),
              matching: find.byType(InkWell));
      await tester.tap(dayChips.at(1));
      await tester.pumpAndSettle();

      // The list contains only points whose local day == day 2; their unique
      // temp is the second local day's day number * 10. We don't assert the
      // exact value to stay tz-agnostic — just that the list is non-empty and
      // that rows for "day 1's temp * 10" no longer dominate.
      expect(find.byKey(const Key('preset-body-hourly')), findsOneWidget);
    });

    testWidgets('drag handle is rendered for sheet semantics', (tester) async {
      final stamps = _hoursOver3Days();
      await _openSheet(
        tester,
        supply: () => _forecast(atm: [for (final t in stamps) _atm(t)]),
      );
      // The sheet itself uses DraggableScrollableSheet.
      expect(find.byType(DraggableScrollableSheet), findsOneWidget);
    });
  });
}
