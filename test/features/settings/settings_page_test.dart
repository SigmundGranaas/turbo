import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/l10n/app_localizations.dart';
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

void main() {
  group('SettingsPage UI Test', () {
    // Before each test, we clear the mock SharedPreferences to ensure
    // a clean and predictable state.
    setUp(() {
      SharedPreferences.setMockInitialValues({});
    });

    testWidgets('loads and displays initial default settings', (WidgetTester tester) async {
      // Arrange: Pump the SettingsPage widget inside our TestApp.
      await tester.pumpWidget(const ProviderScope(child: TestApp(child: SettingsPage())));

      // Act: Wait for the async provider to finish loading from persistence.
      await tester.pumpAndSettle();

      // Assert: Verify all the default UI elements are present.
      expect(find.text('Settings'), findsOneWidget); // AppBar title
      expect(find.text('Theme'), findsOneWidget);
      expect(find.text('Language'), findsOneWidget);

      // Assert: Verify the 'System' theme is selected by default.
      final segmentedButtonTheme = tester.widget<SegmentedButton<ThemeMode>>(find.byType(SegmentedButton<ThemeMode>));
      expect(segmentedButtonTheme.selected, {ThemeMode.system});

      // Assert: Verify the 'English' language is selected by default.
      final segmentedButtonLang = tester.widget<SegmentedButton<Locale>>(find.byType(SegmentedButton<Locale>));
      expect(segmentedButtonLang.selected, {const Locale('en')});
    });

    testWidgets('can change the theme and selection is reflected in the UI', (WidgetTester tester) async {
      // Arrange
      await tester.pumpWidget(const ProviderScope(child: TestApp(child: SettingsPage())));
      await tester.pumpAndSettle();

      // Act: Tap on the 'Dark' theme button's text label.
      await tester.tap(find.text('Dark'));
      await tester.pumpAndSettle();

      // Assert: The selection has updated to Dark.
      var segmentedButtonTheme = tester.widget<SegmentedButton<ThemeMode>>(find.byType(SegmentedButton<ThemeMode>));
      expect(segmentedButtonTheme.selected, {ThemeMode.dark});

      // Act: Tap on the 'Light' theme button's text label.
      await tester.tap(find.text('Light'));
      await tester.pumpAndSettle();

      // Assert: The selection has updated to Light.
      segmentedButtonTheme = tester.widget<SegmentedButton<ThemeMode>>(find.byType(SegmentedButton<ThemeMode>));
      expect(segmentedButtonTheme.selected, {ThemeMode.light});
    });

    testWidgets('can change language and UI text updates accordingly', (WidgetTester tester) async {
      // Arrange
      await tester.pumpWidget(const ProviderScope(child: TestApp(child: SettingsPage())));
      await tester.pumpAndSettle();

      // Assert initial English state
      expect(find.text('Settings'), findsOneWidget);
      expect(find.text('Theme'), findsOneWidget);

      // Act: Tap on 'Norwegian'
      await tester.tap(find.text('Norwegian'));
      await tester.pumpAndSettle();

      // Assert: The selection has changed to Norwegian
      final segmentedButtonLang = tester.widget<SegmentedButton<Locale>>(find.byType(SegmentedButton<Locale>));
      expect(segmentedButtonLang.selected, {const Locale('nb')});

      // Assert: The UI text has updated to Norwegian
      expect(find.text('Innstillinger'), findsOneWidget); // AppBar Title
      expect(find.text('Tema'), findsOneWidget);         // Section Header
      expect(find.text('Språk'), findsOneWidget);        // Section Header
      expect(find.text('Norsk'), findsOneWidget);        // Language Button

      // Act: Tap on 'English' to switch back. We find the Norwegian text for "English".
      await tester.tap(find.text('Engelsk'));
      await tester.pumpAndSettle();

      // Assert: The UI text has updated back to English
      expect(find.text('Settings'), findsOneWidget);
      expect(find.text('Theme'), findsOneWidget);
    });

    testWidgets('settings are persisted and reloaded on subsequent visits', (WidgetTester tester) async {
      // This test simulates closing and reopening the app to check persistence.

      // --- FIRST SESSION ---
      // Arrange & Act: Change settings to non-default values.
      await tester.pumpWidget(const ProviderScope(child: TestApp(child: SettingsPage())));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Dark'));
      await tester.tap(find.text('Norwegian'));
      await tester.pumpAndSettle();

      // Assert that changes were applied in the first session.
      expect(tester.widget<SegmentedButton<ThemeMode>>(find.byType(SegmentedButton<ThemeMode>)).selected, {ThemeMode.dark});
      expect(tester.widget<SegmentedButton<Locale>>(find.byType(SegmentedButton<Locale>)).selected, {const Locale('nb')});

      // --- SECOND SESSION ---
      // Arrange & Act: Re-pump the widget tree. This simulates an app restart.
      // The SharedPreferences mock retains values between pumps in a single test.
      await tester.pumpWidget(const ProviderScope(child: TestApp(child: SettingsPage())));
      await tester.pumpAndSettle();

      // Assert: The loaded state reflects the previously saved settings.
      // The text is in Norwegian, and the button selections are correct.
      expect(find.text('Innstillinger'), findsOneWidget);
      expect(tester.widget<SegmentedButton<ThemeMode>>(find.byType(SegmentedButton<ThemeMode>)).selected, {ThemeMode.dark});
      expect(tester.widget<SegmentedButton<Locale>>(find.byType(SegmentedButton<Locale>)).selected, {const Locale('nb')});
    });
  });
}