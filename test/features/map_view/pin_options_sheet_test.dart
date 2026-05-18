import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/map_view/widgets/pin_options_sheet.dart';
import 'package:turbo/features/search/api.dart';
import 'package:turbo/features/weather/api.dart';

import '../../helpers/pump_app.dart';

class _Callbacks {
  String? lastCreatePrefill;
  int create = 0, measure = 0, navigate = 0, stop = 0;
}

class _StubGeocoder extends KartverketLocationService {
  _StubGeocoder({this.result})
      : super(client: http.Client());
  final LocationSearchResult? result;

  @override
  Future<LocationSearchResult?> findLocationByCoord(LatLng coord,
          {double radiusMeters = 500}) async =>
      result;
}

class _StubWeatherFetcher implements WeatherFetcher {
  @override
  YrAtmosphericService get atmospheric => throw UnimplementedError();
  @override
  YrOceanService get ocean => throw UnimplementedError();

  @override
  Future<WeatherForecast> fetch(LatLng position,
      {WeatherForecast? previous}) async {
    final now = DateTime.now().toUtc();
    return WeatherForecast(
      position: position,
      fetchedAt: now,
      atmosphericExpiresAt: now.add(const Duration(minutes: 30)),
      marineExpiresAt: null,
      atmosphericLastModified: null,
      marineLastModified: null,
      atmospheric: const [],
      marine: const [],
    );
  }
}

Future<_Callbacks> _open(
  WidgetTester tester, {
  bool isNavigating = false,
  LocationSearchResult? geocode,
}) async {
  final cbs = _Callbacks();
  await pumpTestApp(
    tester,
    Builder(
      builder: (ctx) => Center(
        child: ElevatedButton(
          child: const Text('open'),
          onPressed: () => showModalBottomSheet(
            context: ctx,
            isScrollControlled: true,
            useSafeArea: true,
            backgroundColor: Colors.transparent,
            builder: (_) => PinOptionsSheet(
              point: const LatLng(60.39, 5.32),
              isNavigating: isNavigating,
              onCreateMarker: (name) {
                cbs.create++;
                cbs.lastCreatePrefill = name;
              },
              onMeasure: () => cbs.measure++,
              onNavigate: () => cbs.navigate++,
              onStopNavigation: () => cbs.stop++,
            ),
          ),
        ),
      ),
    ),
    overrides: [
      reverseGeocoderProvider
          .overrideWithValue(_StubGeocoder(result: geocode)),
      weatherFetcherProvider.overrideWith((ref) => _StubWeatherFetcher()),
    ],
  );
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
  return cbs;
}

void main() {
  group('PinOptionsSheet', () {
    testWidgets('shows the three action rows with localized labels',
        (tester) async {
      await _open(tester);
      expect(find.text('Create New Marker Here'), findsOneWidget);
      expect(find.text('Measure Distance From Here'), findsOneWidget);
      expect(find.text('Navigate Here'), findsOneWidget);
    });

    testWidgets('when navigation is active, the third row reads "Stop"',
        (tester) async {
      await _open(tester, isNavigating: true);
      expect(find.text('Stop Navigation'), findsOneWidget);
      expect(find.text('Navigate Here'), findsNothing);
    });

    testWidgets('tapping Create marker pops sheet and fires onCreateMarker',
        (tester) async {
      final cbs = await _open(tester);
      await tester.tap(find.text('Create New Marker Here'));
      await tester.pumpAndSettle();

      expect(cbs.create, 1);
      expect(cbs.measure, 0);
      expect(cbs.navigate, 0);
      expect(cbs.stop, 0);
      expect(find.byType(PinOptionsSheet), findsNothing);
    });

    testWidgets('header shows resolved place name', (tester) async {
      await _open(
        tester,
        geocode: LocationSearchResult(
          title: 'Bryggen',
          description: 'tettsted, Bergen',
          position: const LatLng(60.39, 5.32),
          source: 'kartverket',
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Bryggen'), findsOneWidget);
    });

    testWidgets('create marker passes resolved name as prefill',
        (tester) async {
      final cbs = await _open(
        tester,
        geocode: LocationSearchResult(
          title: 'Bryggen',
          position: const LatLng(60.39, 5.32),
          source: 'kartverket',
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Create New Marker Here'));
      await tester.pumpAndSettle();
      expect(cbs.lastCreatePrefill, 'Bryggen');
    });

    testWidgets('tapping Weather tab swaps in the embedded weather body',
        (tester) async {
      await _open(tester);
      await tester.tap(find.byKey(const Key('pin-tab-weather')));
      await tester.pumpAndSettle();
      // After switching tabs the info-body list is no longer mounted.
      expect(find.text('Create New Marker Here'), findsNothing);
    });

    testWidgets('tapping Measure pops sheet and fires onMeasure',
        (tester) async {
      final cbs = await _open(tester);
      await tester.tap(find.text('Measure Distance From Here'));
      await tester.pumpAndSettle();

      expect(cbs.measure, 1);
      expect(find.byType(PinOptionsSheet), findsNothing);
    });

    testWidgets('tapping Navigate (when inactive) fires onNavigate',
        (tester) async {
      final cbs = await _open(tester);
      await tester.tap(find.text('Navigate Here'));
      await tester.pumpAndSettle();

      expect(cbs.navigate, 1);
      expect(cbs.stop, 0);
    });

    testWidgets('tapping Stop Navigation (when active) fires onStopNavigation',
        (tester) async {
      final cbs = await _open(tester, isNavigating: true);
      await tester.tap(find.text('Stop Navigation'));
      await tester.pumpAndSettle();

      expect(cbs.stop, 1);
      expect(cbs.navigate, 0);
    });
  });
}
