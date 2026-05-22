import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/data/database_provider.dart';

/// Builds a minimal app shell suitable for widget tests: ProviderScope +
/// MaterialApp with the app's localization delegates and a Scaffold body.
///
/// Consolidates the per-file `_testApp` / `_pumpSheet` wrappers that
/// previously lived in `path_customization_e2e_test.dart` and
/// `map_layer_button_test.dart`.
Widget buildTestApp(
  Widget child, {
  List<Override> overrides = const [],
  Database? database,
  ThemeData? theme,
  Locale? locale,
}) {
  return ProviderScope(
    overrides: [
      if (database != null)
        databaseProvider.overrideWith((ref) async => database),
      ...overrides,
    ],
    child: MaterialApp(
      theme: theme,
      locale: locale,
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(body: child),
    ),
  );
}

/// Pumps [buildTestApp] into [tester] and (by default) settles animations.
/// Resets `SharedPreferences` to an empty mock store before pumping so tests
/// start from a known state — callers needing seeded prefs should call
/// `SharedPreferences.setMockInitialValues(...)` themselves *after* awaiting
/// this helper. Returns once the first frame is laid out.
Future<void> pumpTestApp(
  WidgetTester tester,
  Widget child, {
  List<Override> overrides = const [],
  Database? database,
  ThemeData? theme,
  Locale? locale,
  bool resetSharedPrefs = true,
  bool settle = true,
}) async {
  if (resetSharedPrefs) {
    SharedPreferences.setMockInitialValues({});
  }
  await tester.pumpWidget(
    buildTestApp(
      child,
      overrides: overrides,
      database: database,
      theme: theme,
      locale: locale,
    ),
  );
  if (settle) {
    await tester.pumpAndSettle();
  }
}

/// Idempotent ffi init for tests that need a SQLite database. Safe to call in
/// every `setUpAll` — `sqfliteFfiInit` is itself idempotent.
void initSqfliteFfi() {
  sqfliteFfiInit();
  databaseFactory = databaseFactoryFfi;
}
