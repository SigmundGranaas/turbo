import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/widgets/app_selection_pill.dart';

Future<void> _pump(WidgetTester tester, Widget child) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: ThemeData.from(
        colorScheme: const ColorScheme.light(),
      ),
      home: Scaffold(body: Center(child: child)),
    ),
  );
}

void main() {
  group('AppSelectionPill', () {
    testWidgets('selected and unselected pills use distinct backgrounds',
        (tester) async {
      await _pump(
        tester,
        const Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            AppSelectionPill(
              key: Key('on'),
              selected: true,
              child: Text('Weather'),
            ),
            AppSelectionPill(
              key: Key('off'),
              selected: false,
              child: Text('Ocean'),
            ),
          ],
        ),
      );

      Material materialUnder(Key k) {
        final inkwell = find.descendant(
          of: find.byKey(k),
          matching: find.byType(InkWell),
        );
        // The InkWell's ancestor Material carries the pill's background.
        final material = find
            .ancestor(of: inkwell, matching: find.byType(Material))
            .first;
        return tester.widget<Material>(material);
      }

      final on = materialUnder(const Key('on'));
      final off = materialUnder(const Key('off'));
      expect(on.color, isNot(equals(off.color)));
    });

    testWidgets('renders without a check icon when selected', (tester) async {
      await _pump(
        tester,
        const AppSelectionPill(
          selected: true,
          child: Text('Weather'),
        ),
      );
      expect(find.byIcon(Icons.check), findsNothing);
    });

    testWidgets('fires onTap when pressed', (tester) async {
      int taps = 0;
      await _pump(
        tester,
        AppSelectionPill(
          selected: false,
          onTap: () => taps++,
          child: const Text('Tap'),
        ),
      );
      await tester.tap(find.text('Tap'));
      expect(taps, 1);
    });

    testWidgets('shows leading icon when provided', (tester) async {
      await _pump(
        tester,
        const AppSelectionPill(
          selected: false,
          leadingIcon: Icons.water,
          child: Text('Ocean'),
        ),
      );
      expect(find.byIcon(Icons.water), findsOneWidget);
    });
  });
}
