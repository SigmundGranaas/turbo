import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/widgets/app_button.dart';

Widget _harness(Widget child) => MaterialApp(home: Scaffold(body: child));

void main() {
  group('AppButton.primary', () {
    testWidgets('renders text and forwards taps', (tester) async {
      var taps = 0;
      await tester.pumpWidget(_harness(AppButton.primary(
        text: 'Save',
        onPressed: () => taps++,
      )));

      expect(find.text('Save'), findsOneWidget);
      await tester.tap(find.byType(AppButton));
      expect(taps, 1);
    });

    testWidgets('is disabled when onPressed is null', (tester) async {
      await tester.pumpWidget(
        _harness(const AppButton.primary(text: 'Save')),
      );
      final FilledButton button = tester.widget(find.byType(FilledButton));
      expect(button.onPressed, isNull);
    });

    testWidgets('shows a progress indicator and rejects taps when loading',
        (tester) async {
      var taps = 0;
      await tester.pumpWidget(_harness(AppButton.primary(
        text: 'Save',
        isLoading: true,
        onPressed: () => taps++,
      )));

      expect(find.text('Save'), findsNothing);
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
      // Tapping while loading must not invoke the callback.
      await tester.tap(find.byType(AppButton), warnIfMissed: false);
      expect(taps, 0);
    });

    testWidgets('fullWidth: true stretches to the parent width',
        (tester) async {
      await tester.pumpWidget(_harness(
        SizedBox(
          width: 320,
          child: AppButton.primary(
              text: 'Save', onPressed: () {}, fullWidth: true),
        ),
      ));
      final size = tester.getSize(find.byType(AppButton));
      expect(size.width, 320);
    });

    testWidgets('default is intrinsic-width (NOT fullWidth)', (tester) async {
      await tester.pumpWidget(_harness(
        SizedBox(
          width: 320,
          child: Center(
            child: AppButton.primary(text: 'Save', onPressed: () {}),
          ),
        ),
      ));
      final size = tester.getSize(find.byType(AppButton));
      // Intrinsic width is much smaller than the 320 parent.
      expect(size.width, lessThan(320));
    });

    testWidgets('icon is rendered alongside text when provided',
        (tester) async {
      await tester.pumpWidget(_harness(AppButton.primary(
        text: 'Download',
        icon: Icons.download,
        onPressed: () {},
      )));
      expect(find.text('Download'), findsOneWidget);
      expect(find.byIcon(Icons.download), findsOneWidget);
    });
  });

  group('AppButton variants', () {
    testWidgets('secondary renders an OutlinedButton', (tester) async {
      await tester.pumpWidget(_harness(
        AppButton.secondary(text: 'Cancel', onPressed: () {}),
      ));
      expect(find.byType(OutlinedButton), findsOneWidget);
    });

    testWidgets('tonal renders a tonal FilledButton', (tester) async {
      await tester.pumpWidget(_harness(
        AppButton.tonal(text: 'Next', onPressed: () {}),
      ));
      expect(find.byType(FilledButton), findsOneWidget);
    });

    testWidgets('danger uses the error color scheme', (tester) async {
      const errorColor = Color(0xFFAA0000);
      await tester.pumpWidget(MaterialApp(
        theme: ThemeData.from(
          colorScheme: const ColorScheme.light(error: errorColor),
        ),
        home: Scaffold(
          body: AppButton.danger(text: 'Delete', onPressed: () {}),
        ),
      ));
      final FilledButton btn = tester.widget(find.byType(FilledButton));
      final bg = btn.style?.backgroundColor?.resolve(<WidgetState>{});
      expect(bg, errorColor);
    });

    testWidgets('text variant renders a TextButton', (tester) async {
      await tester.pumpWidget(_harness(
        AppButton.text(text: 'Forgot?', onPressed: () {}),
      ));
      expect(find.byType(TextButton), findsOneWidget);
    });
  });
}
