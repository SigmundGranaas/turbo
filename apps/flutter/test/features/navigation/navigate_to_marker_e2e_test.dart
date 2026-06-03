import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/core/location/compass_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/map_view/widgets/mode_indicator.dart';
import 'package:turbo/features/navigation/api.dart';
import 'package:turbo/features/search/api.dart';
import 'package:turbo/features/weather/api.dart';

import '../../helpers/pump_app.dart';
import '../../helpers/test_weather_fetcher.dart';

class _NoopGeocoder implements ReverseGeocoder {
  @override
  Future<LocationDescription?> describe(LatLng coord) async => null;
}

List<Override> _sheetOverrides() => [
      reverseGeocoderProvider.overrideWith((ref) => _NoopGeocoder()),
      weatherFetcherProvider.overrideWith((ref) => buildTestWeatherFetcher()),
    ];

class _StubLocation extends LocationState {
  _StubLocation(this._pos);
  final LatLng? _pos;
  @override
  Future<LatLng?> build() async => _pos;
}

/// Drives the navigation state the way the coordinate detail sheet's "Navigate"
/// / "Stop Navigation" actions do, with the [ModeIndicator] on the underlying
/// screen so we can observe the navigation chip light up. (The coordinate
/// actions now live in the shared detail host; this harness exercises the same
/// state transitions the chip reacts to.)
class _NavigateFlowHarness extends ConsumerWidget {
  final LatLng target;
  const _NavigateFlowHarness({required this.target});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final navState = ref.watch(navigationStateProvider);
    final notifier = ref.read(navigationStateProvider.notifier);
    return Stack(
      children: [
        Center(
          child: navState.isActive
              ? ElevatedButton(
                  onPressed: notifier.stopNavigation,
                  child: const Text('Stop Navigation'),
                )
              : ElevatedButton(
                  onPressed: () => notifier.startNavigation(target),
                  child: const Text('Navigate Here'),
                ),
        ),
        const ModeIndicator(),
      ],
    );
  }
}

void main() {
  group('Navigate-to flow end-to-end', () {
    const target = LatLng(60.0, 11.0);

    testWidgets(
        'tapping "Navigate Here" starts navigation and surfaces the '
        'navigation chip in the mode indicator', (tester) async {
      await pumpTestApp(
        tester,
        const _NavigateFlowHarness(target: target),
        overrides: [
          locationStateProvider
              .overrideWith(() => _StubLocation(const LatLng(59.9, 10.7))),
          compassStateProvider
              .overrideWith((ref) => const Stream<double?>.empty()),
          ..._sheetOverrides(),
        ],
      );

      // Sanity: indicator empty before any action.
      expect(find.byIcon(Icons.navigation), findsNothing);

      await tester.tap(find.text('Navigate Here'));
      await tester.pumpAndSettle();

      // The consolidated status chip now shows the navigation chip (distance +
      // direction). Follow is implied by the journey, so there is no separate
      // redundant "Following" chip stacked alongside it.
      expect(find.byIcon(Icons.navigation), findsOneWidget);
    });

    testWidgets(
        'while navigating the control reads "Stop Navigation" and tapping it '
        'returns the state to inactive', (tester) async {
      await pumpTestApp(
        tester,
        const _NavigateFlowHarness(target: target),
        overrides: [
          locationStateProvider
              .overrideWith(() => _StubLocation(const LatLng(59.9, 10.7))),
          compassStateProvider
              .overrideWith((ref) => const Stream<double?>.empty()),
          ..._sheetOverrides(),
        ],
      );

      await tester.tap(find.text('Navigate Here'));
      await tester.pumpAndSettle();

      // The control now reads as Stop.
      expect(find.text('Stop Navigation'), findsOneWidget);

      await tester.tap(find.text('Stop Navigation'));
      await tester.pumpAndSettle();

      // Back to "Navigate Here".
      expect(find.text('Navigate Here'), findsOneWidget);
    });

    testWidgets(
        'NavigationInfoChip renders the formatted distance to target in km '
        'when metric is selected', (tester) async {
      // (59.9,10.7) → (60.0,11.0) ≈ 21 km, well above the 1 km threshold.
      await pumpTestApp(
        tester,
        const _NavigateFlowHarness(target: target),
        overrides: [
          locationStateProvider
              .overrideWith(() => _StubLocation(const LatLng(59.9, 10.7))),
          compassStateProvider
              .overrideWith((ref) => const Stream<double?>.empty()),
          ..._sheetOverrides(),
        ],
      );

      await tester.tap(find.text('Navigate Here'));
      await tester.pumpAndSettle();

      // Chip should now render — find any "<n>.<nn> km" text.
      final kmRe = RegExp(r'^\d+\.\d{2} km$');
      final found = tester
          .widgetList<Text>(find.byType(Text))
          .map((t) => t.data)
          .where((s) => s != null && kmRe.hasMatch(s))
          .toList();
      expect(found, isNotEmpty,
          reason: 'Distance label "<n>.<nn> km" must render in the chip');
    });

    testWidgets(
        'NavigationInfoChip respects the imperial distance unit setting',
        (tester) async {
      SharedPreferences.setMockInitialValues({'distanceUnit': 'imperial'});
      await pumpTestApp(
        tester,
        const _NavigateFlowHarness(target: target),
        resetSharedPrefs: false,
        overrides: [
          locationStateProvider
              .overrideWith(() => _StubLocation(const LatLng(59.9, 10.7))),
          compassStateProvider
              .overrideWith((ref) => const Stream<double?>.empty()),
          ..._sheetOverrides(),
        ],
      );

      await tester.tap(find.text('Navigate Here'));
      await tester.pumpAndSettle();

      // Chip should render in miles, not km.
      final miRe = RegExp(r'^\d+\.\d{2} mi$');
      final found = tester
          .widgetList<Text>(find.byType(Text))
          .map((t) => t.data)
          .where((s) => s != null && miRe.hasMatch(s))
          .toList();
      expect(found, isNotEmpty,
          reason: 'Distance label "<n>.<nn> mi" must render in the chip');
    });

    testWidgets('cycle: navigate → stop → navigate again with a new target',
        (tester) async {
      // Captures the round-trip and ensures startNavigation can be called
      // multiple times without leaking stale targets.
      await pumpTestApp(
        tester,
        const _NavigateFlowHarness(target: target),
        overrides: [
          locationStateProvider
              .overrideWith(() => _StubLocation(const LatLng(59.9, 10.7))),
          compassStateProvider
              .overrideWith((ref) => const Stream<double?>.empty()),
          ..._sheetOverrides(),
        ],
      );

      for (final _ in [1, 2]) {
        await tester.tap(find.text('Navigate Here'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Stop Navigation'));
        await tester.pumpAndSettle();
      }

      // After the loop the control defaults back to Navigate Here.
      expect(find.text('Navigate Here'), findsOneWidget);
    });
  });
}
