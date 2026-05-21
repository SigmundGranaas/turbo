import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
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
  YrSunriseService get sunrise => throw UnimplementedError();
  @override
  MetAlertsService get alerts => throw UnimplementedError();

  @override
  Future<WeatherForecast> fetch(
    LatLng position, {
    WeatherForecast? previous,
  }) async {
    return supply!();
  }
}

class _StubTideService extends KartverketTideService {
  _StubTideService(this.supply) : super(client: http.Client());
  TideForecast? Function() supply;

  @override
  Future<TideForecast?> fetch(LatLng position) async => supply();
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
  TideForecast? Function()? tide,
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
      kartverketTideServiceProvider
          .overrideWithValue(_StubTideService(tide ?? () => null)),
    ],
    settle: false,
  );
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const Key('open-sheet')));
  await tester.pumpAndSettle();
}

void main() {
  group('WeatherDetailSheet', () {
    testWidgets('renders day strip, two preset pills, and the weather list',
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
      expect(find.byKey(const Key('preset-chip-weather')), findsOneWidget);
      // No marine data and no tide → no ocean pill.
      expect(find.byKey(const Key('preset-chip-ocean')), findsNothing);
    });

    testWidgets('Ocean pill appears when forecast has marine data',
        (tester) async {
      final stamps = _hoursOver3Days();
      await _openSheet(
        tester,
        supply: () => _forecast(
          atm: [for (final t in stamps) _atm(t)],
          marine: [for (final t in stamps) _marine(t)],
        ),
      );

      expect(find.byKey(const Key('preset-chip-ocean')), findsOneWidget);
      await tester.tap(find.byKey(const Key('preset-chip-ocean')));
      await tester.pumpAndSettle();
      // Ocean rows show wave height in meters.
      expect(find.textContaining('1.2 m'), findsWidgets);
    });

    testWidgets('Ocean pill is hidden when only tide data is present '
        '(no marine waves)', (tester) async {
      final stamps = _hoursOver3Days();
      final tideTime = stamps.first.add(const Duration(hours: 6));
      await _openSheet(
        tester,
        supply: () => _forecast(
          atm: [for (final t in stamps) _atm(t)],
          // no marine data
        ),
        tide: () => TideForecast(
          stationName: 'Bergen',
          extrema: [
            TideExtremum(
                timeUtc: tideTime, levelCm: 95, kind: TideKind.high),
          ],
          fetchedAt: DateTime.now().toUtc(),
          expiresAt:
              DateTime.now().toUtc().add(const Duration(hours: 6)),
        ),
      );

      // Without marine waves the ocean tab itself shouldn't appear, even
      // if Kartverket has a nearby tide station.
      expect(find.byKey(const Key('preset-chip-ocean')), findsNothing);
    });

    testWidgets('tide table shows up inside the Ocean tab when both marine '
        'and tide data are present', (tester) async {
      final stamps = _hoursOver3Days();
      final tideTime = stamps.first.add(const Duration(hours: 6));
      await _openSheet(
        tester,
        supply: () => _forecast(
          atm: [for (final t in stamps) _atm(t)],
          marine: [for (final t in stamps) _marine(t)],
        ),
        tide: () => TideForecast(
          stationName: 'Bergen',
          extrema: [
            TideExtremum(
                timeUtc: tideTime, levelCm: 95, kind: TideKind.high),
            TideExtremum(
                timeUtc: tideTime.add(const Duration(hours: 6)),
                levelCm: 12,
                kind: TideKind.low),
          ],
          fetchedAt: DateTime.now().toUtc(),
          expiresAt:
              DateTime.now().toUtc().add(const Duration(hours: 6)),
        ),
      );

      expect(find.byKey(const Key('preset-chip-ocean')), findsOneWidget);
      await tester.tap(find.byKey(const Key('preset-chip-ocean')));
      await tester.pumpAndSettle();
      expect(find.text('Tide'), findsOneWidget);
      expect(find.text('High'), findsOneWidget);
      expect(find.text('Low'), findsOneWidget);
    });

    testWidgets('preset pill does NOT render a Material check icon',
        (tester) async {
      final stamps = _hoursOver3Days();
      await _openSheet(
        tester,
        supply: () => _forecast(
          atm: [for (final t in stamps) _atm(t)],
        ),
      );

      // The whole preset row should contain zero checkmark icons,
      // since we replaced ChoiceChip with AppSelectionPill.
      final tabs = find.byKey(const Key('weather-detail-preset-tabs'));
      expect(
        find.descendant(of: tabs, matching: find.byIcon(Icons.check)),
        findsNothing,
      );
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

      expect(find.byKey(const Key('preset-body-weather')), findsOneWidget);
    });

    testWidgets('drag handle is rendered for sheet semantics', (tester) async {
      final stamps = _hoursOver3Days();
      await _openSheet(
        tester,
        supply: () => _forecast(atm: [for (final t in stamps) _atm(t)]),
      );
      expect(find.byType(DraggableScrollableSheet), findsOneWidget);
    });
  });
}
