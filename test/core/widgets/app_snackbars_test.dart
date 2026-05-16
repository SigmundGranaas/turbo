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
    testWidgets('shows the message with a check icon and pill shape',
        (tester) async {
      await tester.pumpWidget(
        _hostFor((ctx) => AppSnackbars.success(ctx, 'Path saved')),
      );

      await tester.tap(find.text('show'));
      await tester.pump(); // start animation
      await tester.pump(const Duration(milliseconds: 50));

      expect(find.text('Path saved'), findsOneWidget);
      expect(find.byIcon(Icons.check_circle), findsOneWidget);

      // SnackBar uses the pill shape on success.
      final snack = tester.widget<SnackBar>(find.byType(SnackBar));
      expect(snack.shape, isA<StadiumBorder>());
      expect(snack.behavior, SnackBarBehavior.floating);
    });
  });

  group('AppSnackbars.error', () {
    testWidgets('shows the message in an errorContainer-tinted snackbar',
        (tester) async {
      const scheme = ColorScheme.light(errorContainer: Color(0xFFFFE0E0));
      await tester.pumpWidget(MaterialApp(
        theme: ThemeData.from(colorScheme: scheme),
        home: Scaffold(
          body: Builder(
            builder: (ctx) => Center(
              child: ElevatedButton(
                onPressed: () => AppSnackbars.error(ctx, 'Network down'),
                child: const Text('show'),
              ),
            ),
          ),
        ),
      ));

      await tester.tap(find.text('show'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 50));

      expect(find.text('Network down'), findsOneWidget);
      // No success-icon on errors — purely text.
      expect(find.byIcon(Icons.check_circle), findsNothing);

      final snack = tester.widget<SnackBar>(find.byType(SnackBar));
      expect(snack.backgroundColor, scheme.errorContainer);
      expect(snack.behavior, SnackBarBehavior.floating);
    });
  });

  group('AppSnackbars.info', () {
    testWidgets('shows the message with no leading icon and no special shape',
        (tester) async {
      await tester.pumpWidget(
        _hostFor((ctx) => AppSnackbars.info(ctx, 'You have arrived')),
      );

      await tester.tap(find.text('show'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 50));

      expect(find.text('You have arrived'), findsOneWidget);
      expect(find.byIcon(Icons.check_circle), findsNothing);
    });
  });
}
