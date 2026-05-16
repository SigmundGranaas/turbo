import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/location/compass_state.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/features/map_view/widgets/mode_indicator.dart';
import 'package:turbo/features/map_view/widgets/pin_options_sheet.dart';
import 'package:turbo/features/navigation/api.dart';

import '../../helpers/pump_app.dart';

class _StubLocation extends LocationState {
  _StubLocation(this._pos);
  final LatLng? _pos;
  @override
  Future<LatLng?> build() async => _pos;
}

/// Hosts a button that opens [PinOptionsSheet] as a proper modal (so
/// `Navigator.pop(context)` inside the sheet's onTaps closes the sheet, not
/// the test route). The [ModeIndicator] sits on the underlying screen so we
/// can observe the navigation chip light up after the user taps "Navigate".
class _NavigateFlowHarness extends ConsumerWidget {
  final LatLng target;
  const _NavigateFlowHarness({required this.target});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final navState = ref.watch(navigationStateProvider);
    return Stack(
      children: [
        Center(
          child: ElevatedButton(
            child: const Text('open sheet'),
            onPressed: () => showModalBottomSheet(
              context: context,
              builder: (_) => PinOptionsSheet(
                isNavigating: navState.isActive,
                onCreateMarker: () {},
                onMeasure: () {},
                onNavigate: () => ref
                    .read(navigationStateProvider.notifier)
                    .startNavigation(target),
                onStopNavigation: () => ref
                    .read(navigationStateProvider.notifier)
                    .stopNavigation(),
              ),
            ),
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
        'Following chip in the mode indicator', (tester) async {
      await pumpTestApp(
        tester,
        const _NavigateFlowHarness(target: target),
        overrides: [
          locationStateProvider
              .overrideWith(() => _StubLocation(const LatLng(59.9, 10.7))),
          compassStateProvider
              .overrideWith((ref) => const Stream<double?>.empty()),
        ],
      );

      // Sanity: indicator empty before any action.
      expect(find.text('Following'), findsNothing);

      await tester.tap(find.text('open sheet'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Navigate Here'));
      await tester.pumpAndSettle();

      // Sheet closed; mode indicator now shows the Following chip because
      // startNavigation flipped followModeProvider.
      expect(find.text('Following'), findsOneWidget);
    });

    testWidgets(
        're-opening the sheet while navigating shows "Stop Navigation" and '
        'tapping it returns the state to inactive', (tester) async {
      await pumpTestApp(
        tester,
        const _NavigateFlowHarness(target: target),
        overrides: [
          locationStateProvider
              .overrideWith(() => _StubLocation(const LatLng(59.9, 10.7))),
          compassStateProvider
              .overrideWith((ref) => const Stream<double?>.empty()),
        ],
      );

      // Start navigation.
      await tester.tap(find.text('open sheet'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Navigate Here'));
      await tester.pumpAndSettle();

      // Re-open the sheet — it now reads as Stop.
      await tester.tap(find.text('open sheet'));
      await tester.pumpAndSettle();
      expect(find.text('Stop Navigation'), findsOneWidget);

      await tester.tap(find.text('Stop Navigation'));
      await tester.pumpAndSettle();

      // Re-open once more — back to "Navigate Here".
      await tester.tap(find.text('open sheet'));
      await tester.pumpAndSettle();
      expect(find.text('Navigate Here'), findsOneWidget);
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
        ],
      );

      for (final _ in [1, 2]) {
        await tester.tap(find.text('open sheet'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Navigate Here'));
        await tester.pumpAndSettle();

        await tester.tap(find.text('open sheet'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Stop Navigation'));
        await tester.pumpAndSettle();
      }

      // After the loop the sheet defaults back to Navigate Here.
      await tester.tap(find.text('open sheet'));
      await tester.pumpAndSettle();
      expect(find.text('Navigate Here'), findsOneWidget);
    });
  });
}
