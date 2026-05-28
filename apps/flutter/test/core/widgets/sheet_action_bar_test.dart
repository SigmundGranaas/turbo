import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/sheet_action_bar.dart';

Widget _harness(Widget child) => MaterialApp(
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(body: Center(child: child)),
    );

SheetAction _action(String label, {bool destructive = false, VoidCallback? onPressed}) =>
    SheetAction(
      icon: Icons.star,
      label: label,
      onPressed: onPressed ?? () {},
      isDestructive: destructive,
    );

void main() {
  group('SheetActionBar', () {
    testWidgets('renders every action inline when within maxInline',
        (tester) async {
      await tester.pumpWidget(_harness(SheetActionBar(actions: [
        _action('One'),
        _action('Two'),
        _action('Three'),
      ])));
      await tester.pumpAndSettle();

      expect(find.text('One'), findsOneWidget);
      expect(find.text('Two'), findsOneWidget);
      expect(find.text('Three'), findsOneWidget);
      expect(find.text('More'), findsNothing);
    });

    testWidgets('collapses the surplus behind a More overflow button',
        (tester) async {
      await tester.pumpWidget(_harness(SheetActionBar(actions: [
        _action('One'),
        _action('Two'),
        _action('Three'),
        _action('Four'),
        _action('Five'),
      ])));
      await tester.pumpAndSettle();

      // With the default maxInline of 4, three actions stay inline and the
      // rest move behind More.
      expect(find.text('One'), findsOneWidget);
      expect(find.text('Two'), findsOneWidget);
      expect(find.text('Three'), findsOneWidget);
      expect(find.text('More'), findsOneWidget);
      expect(find.text('Four'), findsNothing);
      expect(find.text('Five'), findsNothing);
    });

    testWidgets('the More menu exposes the overflow actions and fires them',
        (tester) async {
      var fourTapped = false;
      await tester.pumpWidget(_harness(SheetActionBar(actions: [
        _action('One'),
        _action('Two'),
        _action('Three'),
        _action('Four', onPressed: () => fourTapped = true),
        _action('Five'),
      ])));
      await tester.pumpAndSettle();

      await tester.tap(find.text('More'));
      await tester.pumpAndSettle();

      expect(find.text('Four'), findsOneWidget);
      expect(find.text('Five'), findsOneWidget);

      await tester.tap(find.text('Four'));
      await tester.pumpAndSettle();

      expect(fourTapped, isTrue);
      // The overflow menu dismisses itself once an action runs.
      expect(find.text('Five'), findsNothing);
    });

    testWidgets('respects a custom maxInline', (tester) async {
      await tester.pumpWidget(_harness(SheetActionBar(
        maxInline: 2,
        actions: [
          _action('One'),
          _action('Two'),
          _action('Three'),
        ],
      )));
      await tester.pumpAndSettle();

      // maxInline 2 → one inline action plus the More button.
      expect(find.text('One'), findsOneWidget);
      expect(find.text('More'), findsOneWidget);
      expect(find.text('Two'), findsNothing);
    });

    testWidgets('a null onPressed renders the action disabled', (tester) async {
      await tester.pumpWidget(_harness(SheetActionBar(actions: [
        const SheetAction(
            icon: Icons.delete, label: 'Disabled', onPressed: null),
      ])));
      await tester.pumpAndSettle();

      // Still shown, just not tappable — InkWell with a null callback.
      expect(find.text('Disabled'), findsOneWidget);
      final inkWell = tester.widget<InkWell>(find.byType(InkWell).first);
      expect(inkWell.onTap, isNull);
    });
  });
}
