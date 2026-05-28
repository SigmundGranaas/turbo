import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:flutter_localizations/flutter_localizations.dart';

class TestApp extends ConsumerWidget {
  final Widget child;
  const TestApp({super.key, required this.child});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final settings = ref.watch(settingsProvider);

    return MaterialApp(
      locale: settings.value?.locale,
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      home: child,
    );
  }
}

Future<void> _openAppearance(WidgetTester tester) async {
  await tester.tap(find.text('Appearance'));
  await tester.pumpAndSettle();
}

void main() {
  group('SettingsPage hub', () {
    setUp(() {
      SharedPreferences.setMockInitialValues({});
    });

    testWidgets('shows section tiles for each settings area',
        (WidgetTester tester) async {
      await tester.pumpWidget(
          const ProviderScope(child: TestApp(child: SettingsPage())));
      await tester.pumpAndSettle();

      expect(find.text('Settings'), findsOneWidget); // AppBar title
      expect(find.text('Appearance'), findsOneWidget);
      expect(find.text('Units'), findsOneWidget);
      expect(find.text('My Location'), findsOneWidget);
      expect(find.text('Drawing'), findsOneWidget);
      expect(find.text('Recording'), findsOneWidget);
      expect(find.text('Advanced'), findsOneWidget);
    });

    testWidgets('hub localizes section titles when language changes',
        (WidgetTester tester) async {
      SharedPreferences.setMockInitialValues({'locale': 'nb'});
      await tester.pumpWidget(
          const ProviderScope(child: TestApp(child: SettingsPage())));
      await tester.pumpAndSettle();

      expect(find.text('Innstillinger'), findsOneWidget);
      // Localized section labels reused from existing l10n strings.
      expect(find.text('Avansert'), findsOneWidget);
      expect(find.text('Tegning'), findsOneWidget);
      expect(find.text('Min posisjon'), findsOneWidget);
    });
  });

  group('AppearanceSettingsPage', () {
    setUp(() {
      SharedPreferences.setMockInitialValues({});
    });

    testWidgets('loads with default theme and language selected',
        (WidgetTester tester) async {
      await tester.pumpWidget(
          const ProviderScope(child: TestApp(child: SettingsPage())));
      await tester.pumpAndSettle();
      await _openAppearance(tester);

      expect(find.text('Theme'), findsOneWidget);
      expect(find.text('Language'), findsOneWidget);

      final segmentedButtonTheme = tester.widget<SegmentedButton<ThemeMode>>(
          find.byType(SegmentedButton<ThemeMode>));
      expect(segmentedButtonTheme.selected, {ThemeMode.system});

      final segmentedButtonLang = tester.widget<SegmentedButton<Locale>>(
          find.byType(SegmentedButton<Locale>));
      expect(segmentedButtonLang.selected, {const Locale('en')});
    });

    testWidgets('can change the theme and selection is reflected in the UI',
        (WidgetTester tester) async {
      await tester.pumpWidget(
          const ProviderScope(child: TestApp(child: SettingsPage())));
      await tester.pumpAndSettle();
      await _openAppearance(tester);

      await tester.tap(find.text('Dark'));
      await tester.pumpAndSettle();

      var segmentedButtonTheme = tester.widget<SegmentedButton<ThemeMode>>(
          find.byType(SegmentedButton<ThemeMode>));
      expect(segmentedButtonTheme.selected, {ThemeMode.dark});

      await tester.tap(find.text('Light'));
      await tester.pumpAndSettle();

      segmentedButtonTheme = tester.widget<SegmentedButton<ThemeMode>>(
          find.byType(SegmentedButton<ThemeMode>));
      expect(segmentedButtonTheme.selected, {ThemeMode.light});
    });

    testWidgets('can change language and UI text updates accordingly',
        (WidgetTester tester) async {
      await tester.pumpWidget(
          const ProviderScope(child: TestApp(child: SettingsPage())));
      await tester.pumpAndSettle();
      await _openAppearance(tester);

      expect(find.text('Theme'), findsOneWidget);

      await tester.tap(find.text('Norwegian'));
      await tester.pumpAndSettle();

      final segmentedButtonLang = tester.widget<SegmentedButton<Locale>>(
          find.byType(SegmentedButton<Locale>));
      expect(segmentedButtonLang.selected, {const Locale('nb')});

      expect(find.text('Tema'), findsOneWidget);
      expect(find.text('Språk'), findsOneWidget);
      expect(find.text('Norsk'), findsOneWidget);

      await tester.tap(find.text('Engelsk'));
      await tester.pumpAndSettle();

      expect(find.text('Theme'), findsOneWidget);
    });

    testWidgets('settings are persisted and reloaded on subsequent visits',
        (WidgetTester tester) async {
      // --- FIRST SESSION ---
      await tester.pumpWidget(
          const ProviderScope(child: TestApp(child: SettingsPage())));
      await tester.pumpAndSettle();
      await _openAppearance(tester);
      await tester.tap(find.text('Dark'));
      await tester.tap(find.text('Norwegian'));
      await tester.pumpAndSettle();

      expect(
          tester
              .widget<SegmentedButton<ThemeMode>>(
                  find.byType(SegmentedButton<ThemeMode>))
              .selected,
          {ThemeMode.dark});
      expect(
          tester
              .widget<SegmentedButton<Locale>>(
                  find.byType(SegmentedButton<Locale>))
              .selected,
          {const Locale('nb')});

      // --- SECOND SESSION ---
      // Use UniqueKey to force a fresh element tree; pumpWidget otherwise
      // reuses the Navigator and keeps the pushed route from session 1.
      await tester.pumpWidget(ProviderScope(
          key: UniqueKey(), child: const TestApp(child: SettingsPage())));
      await tester.pumpAndSettle();
      // The hub should now show the Norwegian title.
      expect(find.text('Innstillinger'), findsOneWidget);

      // Section labels are not localized yet; still in English.
      await tester.tap(find.text('Appearance'));
      await tester.pumpAndSettle();

      expect(
          tester
              .widget<SegmentedButton<ThemeMode>>(
                  find.byType(SegmentedButton<ThemeMode>))
              .selected,
          {ThemeMode.dark});
      expect(
          tester
              .widget<SegmentedButton<Locale>>(
                  find.byType(SegmentedButton<Locale>))
              .selected,
          {const Locale('nb')});
    });
  });
}
