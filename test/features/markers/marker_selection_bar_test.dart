import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/markers/api.dart';

import '../../helpers/pump_app.dart';

/// Fake LocationRepository for widget-level tests. Records bulk-delete
/// invocations so the behavioral assertion can verify the bar actually
/// reaches the repository with the right uuids. The real repository's
/// sqflite-backed deleteMarkers is covered in bulk_actions_test.dart.
class _FakeRepo extends LocationRepository {
  final List<List<String>> deleteCalls = [];

  @override
  AsyncValue<List<Marker>> build() => const AsyncValue.data([]);

  @override
  Future<void> deleteMarkers(List<String> uuids) async {
    deleteCalls.add(List.of(uuids));
  }

  @override
  Future<void> deleteMarker(String uuid) async {
    deleteCalls.add([uuid]);
  }
}

/// `pumpAndSettle` waits for the success snackbar's 4 s auto-dismiss timer.
/// We don't care about the auto-dismiss in these tests — we just need the
/// bar's state transitions to flush — so pump in small bounded bursts until
/// the predicate is true.
Future<void> _pumpUntil(
  WidgetTester tester,
  bool Function() predicate, {
  Duration step = const Duration(milliseconds: 50),
  int maxIterations = 40,
}) async {
  for (var i = 0; i < maxIterations; i++) {
    if (predicate()) return;
    await tester.pump(step);
  }
  await tester.pump(step);
}

void main() {
  group('MarkerSelectionBar rendering', () {
    testWidgets('renders nothing when the selection is empty',
        (tester) async {
      await pumpTestApp(tester, const MarkerSelectionBar());
      expect(find.byType(IconButton), findsNothing);
      expect(find.textContaining('selected'), findsNothing);
    });

    testWidgets(
        'shows the selection count and the Cancel / Export / Delete icons '
        'when at least one marker is selected', (tester) async {
      await pumpTestApp(tester, const MarkerSelectionBar());
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MarkerSelectionBar)),
      );

      container.read(markerSelectionProvider.notifier).toggle('a');
      container.read(markerSelectionProvider.notifier).toggle('b');
      await tester.pumpAndSettle();

      expect(find.text('2 selected'), findsOneWidget);
      expect(find.byIcon(Icons.close), findsOneWidget);
      expect(find.byIcon(Icons.ios_share_outlined), findsOneWidget);
      expect(find.byIcon(Icons.delete_outline), findsOneWidget);
    });

    testWidgets('Cancel icon clears the selection and the bar disappears',
        (tester) async {
      await pumpTestApp(tester, const MarkerSelectionBar());
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MarkerSelectionBar)),
      );
      container.read(markerSelectionProvider.notifier).toggle('x');
      await tester.pumpAndSettle();

      expect(find.text('1 selected'), findsOneWidget);

      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();

      expect(container.read(markerSelectionProvider), isEmpty);
      expect(find.text('1 selected'), findsNothing);
    });
  });

  group('MarkerSelectionBar delete flow', () {
    /// Mounts the bar with a fake repository so the bar's behavior can be
    /// asserted without dragging in sqflite / connectivity / auth. The
    /// real repository's deleteMarkers semantics are covered in
    /// bulk_actions_test.dart.
    Future<({ProviderContainer container, _FakeRepo repo})> setUpBar(
      WidgetTester tester,
      Set<String> initialSelection,
    ) async {
      final fakeRepo = _FakeRepo();
      await pumpTestApp(
        tester,
        const MarkerSelectionBar(),
        overrides: [
          locationRepositoryProvider.overrideWith(() => fakeRepo),
        ],
      );
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MarkerSelectionBar)),
      );
      for (final uuid in initialSelection) {
        container.read(markerSelectionProvider.notifier).toggle(uuid);
      }
      await tester.pumpAndSettle();
      return (container: container, repo: fakeRepo);
    }

    testWidgets(
        'Delete icon opens the destructive confirmation dialog with the '
        'selection count in the message', (tester) async {
      await setUpBar(tester, {'a', 'b', 'c'});

      await tester.tap(find.byIcon(Icons.delete_outline));
      await tester.pumpAndSettle();

      expect(find.text('Delete selected markers?'), findsOneWidget);
      expect(find.textContaining('3 marker'), findsOneWidget);
      expect(find.widgetWithText(TextButton, 'Cancel'), findsOneWidget);
      expect(find.widgetWithText(FilledButton, 'Delete'), findsOneWidget);
    });

    testWidgets(
        'confirming the dialog calls deleteMarkers with every selected '
        'uuid, clears the selection, and surfaces the success snackbar',
        (tester) async {
      final ctx = await setUpBar(tester, {'drop-1', 'drop-2'});
      expect(find.text('2 selected'), findsOneWidget);

      await tester.tap(find.byIcon(Icons.delete_outline));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'Delete'));

      // Pump until the snackbar's "Markers deleted" text actually mounts.
      // pumpAndSettle would wait through its 4 s auto-dismiss timer; the
      // bounded pump lets the SnackBar's slide-in animation reach the
      // visible state and stops there.
      await _pumpUntil(
        tester,
        () => find.text('Markers deleted').evaluate().isNotEmpty,
      );

      // Behavior 1: repository received exactly one bulk-delete call with
      // the two selected uuids.
      expect(ctx.repo.deleteCalls, hasLength(1));
      expect(ctx.repo.deleteCalls.single.toSet(), {'drop-1', 'drop-2'});

      // Behavior 2: selection set was cleared after the delete completed.
      expect(ctx.container.read(markerSelectionProvider), isEmpty);

      // Behavior 3: success snackbar surfaced.
      expect(find.text('Markers deleted'), findsOneWidget);

      // Behavior 4: with the selection empty, the bar no longer renders
      // its count label.
      expect(find.text('2 selected'), findsNothing);

      // Drain the snackbar's auto-dismiss timer so the test exits cleanly.
      await tester.pumpAndSettle(const Duration(seconds: 6));
    });

    testWidgets(
        'cancelling the destructive dialog leaves the selection intact and '
        'does NOT call the repository', (tester) async {
      final ctx = await setUpBar(tester, {'keep'});

      await tester.tap(find.byIcon(Icons.delete_outline));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();

      expect(ctx.container.read(markerSelectionProvider), {'keep'});
      expect(ctx.repo.deleteCalls, isEmpty);
      // Bar still visible.
      expect(find.text('1 selected'), findsOneWidget);
    });
  });
}
