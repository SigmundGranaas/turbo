import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/search/data/composite_search_service.dart';
import 'package:turbo/features/search/data/location_service.dart';
import 'package:turbo/features/search/widgets/search_bar_mobile.dart';

import '../../helpers/fakes/fake_location_service.dart';
import '../../helpers/pump_app.dart';

LocationSearchResult _r(String title, String source) => LocationSearchResult(
      title: title,
      position: const LatLng(59.9, 10.7),
      source: source,
    );

/// Thin [CompositeSearchService] stand-in backed by a single [FakeLocationService].
class _FakeComposite extends CompositeSearchService {
  final LocationService inner;
  _FakeComposite(this.inner) : super(inner, inner, inner);
  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) =>
      inner.findLocationsBy(name);
}

/// Hosts [MobileSearchBar] alongside a minimal [FlutterMap] so the controller
/// passed in is attached (suggestion taps invoke `animatedMapMove`, which reads
/// `mapController.camera`).
class _Host extends StatefulWidget {
  final VoidCallback onMenu;
  const _Host({required this.onMenu});

  @override
  State<_Host> createState() => _HostState();
}

class _HostState extends State<_Host> with TickerProviderStateMixin {
  late final MapController _mapController = MapController();

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        SizedBox(
          width: 800,
          height: 600,
          child: FlutterMap(
            mapController: _mapController,
            options: const MapOptions(
              initialCenter: LatLng(59.9, 10.7),
              initialZoom: 6,
            ),
            children: const [],
          ),
        ),
        Positioned(
          top: 24,
          left: 0,
          right: 0,
          child: MobileSearchBar(
            mapController: _mapController,
            tickerProvider: this,
            onMenuPressed: widget.onMenu,
          ),
        ),
      ],
    );
  }
}

void main() {
  group('MobileSearchBar end-to-end', () {
    testWidgets('typing a query renders matching suggestions after debounce',
        (tester) async {
      final service = FakeLocationService(results: [
        _r('Oslo', 'kartverket'),
        _r('Oslofjorden', 'kartverket'),
      ]);

      await pumpTestApp(
        tester,
        _Host(onMenu: () {}),
        overrides: [
          compositeSearchServiceProvider
              .overrideWithValue(_FakeComposite(service)),
        ],
      );

      await tester.tap(find.byType(TextField));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'oslo');
      // Debounce is 400 ms. Pump a few frames past it.
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pumpAndSettle();

      expect(find.text('Oslo'), findsOneWidget);
      expect(find.text('Oslofjorden'), findsOneWidget);
      expect(service.queries, ['oslo']);
    });

    testWidgets('empty result set renders the "No results found" message',
        (tester) async {
      final service = FakeLocationService(results: const []);

      await pumpTestApp(
        tester,
        _Host(onMenu: () {}),
        overrides: [
          compositeSearchServiceProvider
              .overrideWithValue(_FakeComposite(service)),
        ],
      );

      await tester.tap(find.byType(TextField));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'zzz');
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pumpAndSettle();

      expect(find.text('No results found.'), findsOneWidget);
    });

    testWidgets('service errors surface in the suggestion overlay '
        '— UX gap: currently a raw "Error: <toString>" string',
        (tester) async {
      // **Light fix candidate.** The mobile bar prints `Error: $err` verbatim
      // into the overlay (`search_bar_mobile.dart:236-241`). That's untranslated
      // and not user-friendly. This test pins the current behavior; a follow-up
      // should swap it for an l10n string + retry affordance.
      final service =
          FakeLocationService(throwOnQuery: StateError('network down'));

      await pumpTestApp(
        tester,
        _Host(onMenu: () {}),
        overrides: [
          compositeSearchServiceProvider
              .overrideWithValue(_FakeComposite(service)),
        ],
      );

      await tester.tap(find.byType(TextField));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'oslo');
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pumpAndSettle();

      expect(find.textContaining('Error:'), findsOneWidget);
    });

    testWidgets('clear icon appears when text is non-empty and empties the '
        'field when pressed', (tester) async {
      final service = FakeLocationService();

      await pumpTestApp(
        tester,
        _Host(onMenu: () {}),
        overrides: [
          compositeSearchServiceProvider
              .overrideWithValue(_FakeComposite(service)),
        ],
      );

      await tester.tap(find.byType(TextField));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'oslo');
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pumpAndSettle();

      // The clear icon (Icons.clear) is now visible because the controller has
      // text. Tap it and verify the field empties.
      expect(find.byIcon(Icons.clear), findsOneWidget);
      await tester.tap(find.byIcon(Icons.clear));
      await tester.pumpAndSettle();
      // Wait for the focus-loss-delay to dismiss the overlay (200 ms in the
      // implementation) to avoid trailing-state asserts on disposed timers.
      await tester.pump(const Duration(milliseconds: 250));

      final field = tester.widget<TextField>(find.byType(TextField));
      expect(field.controller!.text, isEmpty);
    });

    testWidgets('queries < 2 chars do not hit the service', (tester) async {
      final service = FakeLocationService();

      await pumpTestApp(
        tester,
        _Host(onMenu: () {}),
        overrides: [
          compositeSearchServiceProvider
              .overrideWithValue(_FakeComposite(service)),
        ],
      );

      await tester.tap(find.byType(TextField));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'a');
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pumpAndSettle();

      expect(service.queries, isEmpty);
    });

    testWidgets('menu icon button invokes onMenuPressed', (tester) async {
      var menuPresses = 0;
      await pumpTestApp(
        tester,
        _Host(onMenu: () => menuPresses++),
        overrides: [
          compositeSearchServiceProvider
              .overrideWithValue(_FakeComposite(FakeLocationService())),
        ],
      );

      await tester.tap(find.byIcon(Icons.menu));
      await tester.pumpAndSettle();
      expect(menuPresses, 1);
    });
  });
}
