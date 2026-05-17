import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/markers/api.dart';

import '../../helpers/pump_app.dart';

void main() {
  group('MarkerSelectionBar', () {
    testWidgets('renders nothing when the selection is empty',
        (tester) async {
      await pumpTestApp(
        tester,
        const MarkerSelectionBar(),
      );
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

      // Count label.
      expect(find.text('2 selected'), findsOneWidget);
      // Three icon actions.
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

    testWidgets(
        'Delete icon opens the destructive confirmation dialog with a '
        'message that includes the selection count', (tester) async {
      await pumpTestApp(tester, const MarkerSelectionBar());
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MarkerSelectionBar)),
      );
      container.read(markerSelectionProvider.notifier)
        ..toggle('a')
        ..toggle('b')
        ..toggle('c');
      await tester.pumpAndSettle();

      await tester.tap(find.byIcon(Icons.delete_outline));
      await tester.pumpAndSettle();

      expect(find.text('Delete selected markers?'), findsOneWidget);
      // Message embeds the count.
      expect(find.textContaining('3 marker'), findsOneWidget);
      // Cancel + Delete actions.
      expect(find.widgetWithText(TextButton, 'Cancel'), findsOneWidget);
      expect(find.widgetWithText(FilledButton, 'Delete'), findsOneWidget);
    });

    testWidgets('cancelling the destructive dialog leaves the selection '
        'unchanged', (tester) async {
      await pumpTestApp(tester, const MarkerSelectionBar());
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MarkerSelectionBar)),
      );
      container.read(markerSelectionProvider.notifier).toggle('keep');
      await tester.pumpAndSettle();

      await tester.tap(find.byIcon(Icons.delete_outline));
      await tester.pumpAndSettle();

      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();

      // Selection still in place; bar still visible.
      expect(container.read(markerSelectionProvider), {'keep'});
      expect(find.text('1 selected'), findsOneWidget);
    });
  });
}
