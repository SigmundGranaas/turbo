import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/widgets/app_list_card.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

/// Test double for [SettingsNotifier]. Records calls; doesn't touch
/// SharedPreferences, the documents dir, or anything else.
class _FakeSettings extends SettingsNotifier {
  int resetCallCount = 0;

  @override
  Future<SettingsState> build() async => const SettingsState(
        themeMode: ThemeMode.system,
        locale: Locale('en'),
        drawSensitivity: 15.0,
        smoothLine: true,
        showIntermediatePoints: false,
      );

  @override
  Future<void> resetLocationIcon() async {
    resetCallCount++;
  }
}

Future<_FakeSettings> _open(WidgetTester tester) async {
  final fake = _FakeSettings();
  await tester.pumpWidget(
    ProviderScope(
      overrides: [settingsProvider.overrideWith(() => fake)],
      child: MaterialApp(
        localizationsDelegates: const [
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: Consumer(builder: (ctx, ref, _) {
            return Center(
              child: ElevatedButton(
                child: const Text('open'),
                onPressed: () => showLocationIconPickerSheet(ctx, ref),
              ),
            );
          }),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
  return fake;
}

void main() {
  group('LocationIconPickerSheet (flattened layout)', () {
    testWidgets('shows the four flat options as AppListCard rows',
        (tester) async {
      await _open(tester);

      // No nested ActionButton tiles, no "Colors" indirection.
      expect(find.byType(AppListCard), findsNWidgets(4));
      expect(find.byIcon(Icons.grid_view), findsOneWidget);
      expect(find.byIcon(Icons.photo_library), findsOneWidget);
      expect(find.byIcon(Icons.camera_alt), findsOneWidget);
      expect(find.byIcon(Icons.restart_alt), findsOneWidget);
    });

    testWidgets('Reset row pops the sheet and calls resetLocationIcon',
        (tester) async {
      final fake = await _open(tester);

      // Localized "Reset to Default" — find the AppListCard by its icon
      // rather than the label so we don't depend on the exact wording.
      await tester.tap(find.byIcon(Icons.restart_alt));
      await tester.pumpAndSettle();

      expect(fake.resetCallCount, 1);
      // Sheet closes after reset.
      expect(find.byType(AppListCard), findsNothing);
    });

    testWidgets('close icon pops the sheet without firing any action',
        (tester) async {
      final fake = await _open(tester);

      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();

      expect(fake.resetCallCount, 0);
      expect(find.byType(AppListCard), findsNothing);
    });
  });
}
