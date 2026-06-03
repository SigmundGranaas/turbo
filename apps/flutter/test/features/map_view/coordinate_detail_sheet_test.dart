import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/map_view/widgets/map_entity_detail_sheet.dart';
import 'package:turbo/features/search/api.dart';
import 'package:turbo/features/weather/api.dart';

import '../../helpers/pump_app.dart';
import '../../helpers/test_weather_fetcher.dart';

class _StubGeocoder implements ReverseGeocoder {
  _StubGeocoder({this.description});
  final LocationDescription? description;
  @override
  Future<LocationDescription?> describe(LatLng coord) async => description;
}

/// Builds a coordinate selection mirroring `MainMapPage._coordinateActions`:
/// the rich place-info + weather body, its own action set (no standard entity
/// actions), short labels to match the icon-over-label bar.
MapSelection _coordinateSelection(
  LatLng point, {
  required VoidCallback onNavigate,
  required VoidCallback onMarker,
}) {
  return MapSelection(
    point: point,
    title: 'Selected location',
    includeStandardActions: false,
    bodyBuilder: (_) => CoordinateDetailBody(point: point),
    extraActions: [
      MapEntityAction(
        id: 'coord_navigate',
        label: 'Navigate',
        icon: Icons.navigation_outlined,
        isAvailable: (_) => true,
        invoke: (c) {
          c.afterJourneyAction?.call();
          onNavigate();
        },
      ),
      MapEntityAction(
        id: 'coord_marker',
        label: 'Marker',
        icon: Icons.add_location_alt_outlined,
        isAvailable: (_) => true,
        invoke: (c) {
          c.afterJourneyAction?.call();
          onMarker();
        },
      ),
    ],
  );
}

Future<void> _open(
  WidgetTester tester, {
  LocationDescription? description,
  VoidCallback? onNavigate,
  VoidCallback? onMarker,
}) async {
  await pumpTestApp(
    tester,
    Builder(
      builder: (ctx) => Center(
        child: ElevatedButton(
          child: const Text('open'),
          onPressed: () => showModalBottomSheet<void>(
            context: ctx,
            isScrollControlled: true,
            useSafeArea: true,
            backgroundColor: Colors.transparent,
            builder: (_) => MapEntityDetailSheet(
              selection: _coordinateSelection(
                const LatLng(60.39, 5.32),
                onNavigate: onNavigate ?? () {},
                onMarker: onMarker ?? () {},
              ),
            ),
          ),
        ),
      ),
    ),
    overrides: [
      reverseGeocoderProvider
          .overrideWith((ref) => _StubGeocoder(description: description)),
      weatherFetcherProvider.overrideWith((ref) => buildTestWeatherFetcher()),
    ],
  );
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
}

void main() {
  group('Coordinate detail sheet (selection seam)', () {
    testWidgets('renders the place-info body and weather surface',
        (tester) async {
      await _open(tester);
      // Falls back to the friendly label when reverse-geo returns null.
      expect(find.text('Selected location'), findsOneWidget);
      expect(
          find.byKey(const Key('pin-sheet-weather-surface')), findsOneWidget);
    });

    testWidgets('header shows the resolved title with qualifier prefix',
        (tester) async {
      await _open(
        tester,
        description: const LocationDescription(
          title: 'Galdhøpiggen',
          qualifier: LocationQualifier.on,
          secondary: 'Lom, Innlandet',
          distanceMeters: 24,
        ),
      );
      expect(find.text('On Galdhøpiggen'), findsOneWidget);
    });

    testWidgets('renders the coordinate actions through the shared bar',
        (tester) async {
      await _open(tester);
      expect(find.text('Navigate'), findsOneWidget);
      expect(find.text('Marker'), findsOneWidget);
    });

    testWidgets('tapping an action fires its callback and closes the sheet',
        (tester) async {
      var navigated = 0;
      await _open(tester, onNavigate: () => navigated++);
      await tester.tap(find.text('Navigate'));
      await tester.pumpAndSettle();
      expect(navigated, 1);
      expect(find.byType(MapEntityDetailSheet), findsNothing);
    });
  });
}
