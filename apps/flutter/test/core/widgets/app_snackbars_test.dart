import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';

Widget _hostFor(void Function(BuildContext) onPressed) {
  return MaterialApp(
    home: Scaffold(
      body: Builder(
        builder: (ctx) => Center(
          child: ElevatedButton(
            onPressed: () => onPressed(ctx),
            child: const Text('show'),
          ),
        ),
      ),
    ),
  );
}

void main() {
  group('AppSnackbars.success', () {
    testWidgets('shows the message with a check icon on the inverse surface',
        (tester) async {
      await tester.pumpWidget(
        _hostFor((ctx) => AppSnackbars.success(ctx, 'Path saved')),
      );

      await tester.tap(find.text('show'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 50));

      expect(find.text('Path saved'), findsOneWidget);
      expect(find.byIcon(Icons.check_circle_outline), findsOneWidget);

      final snack = tester.widget<SnackBar>(find.byType(SnackBar));
      expect(snack.shape, isA<RoundedRectangleBorder>());
      expect(snack.behavior, SnackBarBehavior.floating);
    });
  });

  group('AppSnackbars.error', () {
    testWidgets('shows the message with an error icon on the inverse surface',
        (tester) async {
      await tester.pumpWidget(
        _hostFor((ctx) => AppSnackbars.error(ctx, 'Network down')),
      );

      await tester.tap(find.text('show'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 50));

      expect(find.text('Network down'), findsOneWidget);
      expect(find.byIcon(Icons.error_outline), findsOneWidget);
      // Unified surface — never the success check.
      expect(find.byIcon(Icons.check_circle_outline), findsNothing);

      final snack = tester.widget<SnackBar>(find.byType(SnackBar));
      expect(snack.behavior, SnackBarBehavior.floating);
    });
  });

  group('AppSnackbars.info', () {
    testWidgets('shows the message with an info icon', (tester) async {
      await tester.pumpWidget(
        _hostFor((ctx) => AppSnackbars.info(ctx, 'You have arrived')),
      );

      await tester.tap(find.text('show'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 50));

      expect(find.text('You have arrived'), findsOneWidget);
      expect(find.byIcon(Icons.info_outline), findsOneWidget);
      expect(find.byIcon(Icons.check_circle_outline), findsNothing);
    });

    testWidgets('clears any current snackbar so they never stack',
        (tester) async {
      await tester.pumpWidget(
        _hostFor((ctx) {
          AppSnackbars.info(ctx, 'first');
          AppSnackbars.success(ctx, 'second');
        }),
      );

      await tester.tap(find.text('show'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 50));

      // Only the latest survives — no overlap.
      expect(find.byType(SnackBar), findsOneWidget);
      expect(find.text('second'), findsOneWidget);
      expect(find.text('first'), findsNothing);
    });
  });
}
