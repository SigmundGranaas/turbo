import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/map_view/widgets/pin_options_sheet.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

class _Callbacks {
  int create = 0, measure = 0, navigate = 0, stop = 0;
}

Future<_Callbacks> _open(WidgetTester tester, {bool isNavigating = false}) async {
  final cbs = _Callbacks();
  await tester.pumpWidget(
    MaterialApp(
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(
        body: Builder(
          builder: (ctx) => Center(
            child: ElevatedButton(
              child: const Text('open'),
              onPressed: () => showModalBottomSheet(
                context: ctx,
                builder: (_) => PinOptionsSheet(
                  isNavigating: isNavigating,
                  onCreateMarker: () => cbs.create++,
                  onMeasure: () => cbs.measure++,
                  onNavigate: () => cbs.navigate++,
                  onStopNavigation: () => cbs.stop++,
                ),
              ),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
  return cbs;
}

void main() {
  group('PinOptionsSheet', () {
    testWidgets('shows the three action rows with localized labels',
        (tester) async {
      await _open(tester);
      expect(find.text('Create New Marker Here'), findsOneWidget);
      expect(find.text('Measure Distance From Here'), findsOneWidget);
      expect(find.text('Navigate Here'), findsOneWidget);
    });

    testWidgets('when navigation is active, the third row reads "Stop"',
        (tester) async {
      await _open(tester, isNavigating: true);
      expect(find.text('Stop Navigation'), findsOneWidget);
      expect(find.text('Navigate Here'), findsNothing);
    });

    testWidgets('tapping Create marker pops sheet and fires onCreateMarker',
        (tester) async {
      final cbs = await _open(tester);
      await tester.tap(find.text('Create New Marker Here'));
      await tester.pumpAndSettle();

      expect(cbs.create, 1);
      expect(cbs.measure, 0);
      expect(cbs.navigate, 0);
      expect(cbs.stop, 0);
      expect(find.byType(PinOptionsSheet), findsNothing);
    });

    testWidgets('tapping Measure pops sheet and fires onMeasure',
        (tester) async {
      final cbs = await _open(tester);
      await tester.tap(find.text('Measure Distance From Here'));
      await tester.pumpAndSettle();

      expect(cbs.measure, 1);
      expect(find.byType(PinOptionsSheet), findsNothing);
    });

    testWidgets('tapping Navigate (when inactive) fires onNavigate',
        (tester) async {
      final cbs = await _open(tester);
      await tester.tap(find.text('Navigate Here'));
      await tester.pumpAndSettle();

      expect(cbs.navigate, 1);
      expect(cbs.stop, 0);
    });

    testWidgets('tapping Stop Navigation (when active) fires onStopNavigation',
        (tester) async {
      final cbs = await _open(tester, isNavigating: true);
      await tester.tap(find.text('Stop Navigation'));
      await tester.pumpAndSettle();

      expect(cbs.stop, 1);
      expect(cbs.navigate, 0);
    });
  });
}
