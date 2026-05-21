import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/connectivity/connectivity_provider.dart';
import 'package:turbo/features/search/data/composite_search_service.dart';
import 'package:turbo/features/search/data/location_service.dart';
import 'package:turbo/features/search/widgets/search_bar_mobile.dart';

import '../../helpers/fakes/fake_location_service.dart';
import '../../helpers/pump_app.dart';

/// Stub that lets the test pin connectivity to a chosen boolean — the real
/// ConnectivityNotifier subscribes to a stream we don't want in unit tests.
class _StubConnectivity extends ConnectivityNotifier {
  _StubConnectivity(this._initial);
  final bool _initial;
  @override
  bool build() => _initial;
}

LocationSearchResult _r(String title, String source) => LocationSearchResult(
      title: title,
      position: const LatLng(59.9, 10.7),
      source: source,
    );

/// Thin [CompositeSearchService] stand-in backed by a single [FakeLocationService].
class _FakeComposite extends CompositeSearchService {
  final LocationService inner;
  _FakeComposite(this.inner) : super(inner, inner, inner, inner);
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

    testWidgets('service errors surface a localized message with a Retry '
        'affordance, and tapping Retry re-issues the query', (tester) async {
      var failNext = true;
      final service = FakeLocationService(
        responder: (q) async {
          if (failNext) {
            failNext = false;
            throw StateError('network down');
          }
          return [_r('Oslo', 'kartverket')];
        },
      );

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

      // Friendly localized error — not "Error: <stacktrace>".
      expect(
          find.text('Search failed. Check your connection and try again.'),
          findsOneWidget);
      expect(find.text('Retry'), findsOneWidget);
      expect(find.textContaining('Error:'), findsNothing);

      // Tapping Retry kicks off another search; the responder succeeds this
      // time and the overlay shows the matching suggestion.
      await tester.tap(find.text('Retry'));
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pumpAndSettle();

      expect(find.text('Oslo'), findsOneWidget);
      // Two queries: the original failure + the retry.
      expect(service.queries, ['oslo', 'oslo']);
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

    testWidgets('offline banner appears in the suggestion overlay when '
        'connectivity is false', (tester) async {
      await pumpTestApp(
        tester,
        _Host(onMenu: () {}),
        overrides: [
          compositeSearchServiceProvider
              .overrideWithValue(_FakeComposite(FakeLocationService())),
          connectivityProvider.overrideWith(() => _StubConnectivity(false)),
        ],
      );

      await tester.tap(find.byType(TextField));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'os');
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pumpAndSettle();

      expect(
          find.textContaining('only saved markers and paths'), findsOneWidget,
          reason: 'offline hint should render when connectivityProvider == false');
    });

    testWidgets('offline banner is absent when online', (tester) async {
      await pumpTestApp(
        tester,
        _Host(onMenu: () {}),
        overrides: [
          compositeSearchServiceProvider
              .overrideWithValue(_FakeComposite(FakeLocationService())),
          connectivityProvider.overrideWith(() => _StubConnectivity(true)),
        ],
      );

      await tester.tap(find.byType(TextField));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'os');
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pumpAndSettle();

      expect(find.textContaining('only saved markers and paths'), findsNothing);
    });
  });
}
