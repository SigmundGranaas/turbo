import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/features/weather/api.dart';
import 'package:turbo/features/weather/data/sqlite_marker_weather_prefs_store.dart';

import '../../helpers/pump_app.dart';

const _tableDdl = '''
  CREATE TABLE marker_weather_prefs(
    marker_uuid TEXT PRIMARY KEY,
    metrics     TEXT NOT NULL
  )
''';

Future<Database> _openDb() async {
  initSqfliteFfi();
  final db = await databaseFactory.openDatabase(inMemoryDatabasePath);
  await db.execute(_tableDdl);
  return db;
}

void main() {
  group('WeatherMetricsSheet', () {
    late Database db;
    late MarkerWeatherPrefsStore store;

    setUp(() async {
      db = await _openDb();
      store = SQLiteMarkerWeatherPrefsStore(db);
    });

    tearDown(() async {
      await db.close();
    });

    Future<void> pumpSheet(WidgetTester tester) async {
      // Tall viewport so the whole sheet (11 metrics + header + buttons) fits
      // on-screen and tap() doesn't need scroll gymnastics.
      tester.view.physicalSize = const Size(800, 1800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(() {
        tester.view.resetPhysicalSize();
        tester.view.resetDevicePixelRatio();
      });
      // Render the sheet directly — skipping showModalBottomSheet keeps the
      // test independent of the modal animation pipeline and makes ancestry-
      // based finders simpler. Production code paths still flow through
      // showModalBottomSheet (verified by the marker_info_sheet integration
      // test); this widget contains all its UI in `build` and doesn't depend
      // on a parent ModalRoute.
      await pumpTestApp(
        tester,
        const WeatherMetricsSheet(markerUuid: 'marker-1'),
        overrides: [
          markerWeatherPrefsStoreProvider.overrideWith((ref) async => store),
        ],
        settle: false,
      );
      await tester.pumpAndSettle();
    }

    testWidgets('renders atmospheric and marine sections', (tester) async {
      await pumpSheet(tester);
      expect(find.text('Atmospheric'), findsOneWidget);
      expect(find.text('Marine'), findsOneWidget);
      expect(find.text('Temperature'), findsOneWidget);
      expect(find.text('Wave height'), findsOneWidget);
    });

    testWidgets('checks the defaults: temperature, wind, precipitation',
        (tester) async {
      await pumpSheet(tester);
      // Defaults are checked. Snow is not.
      final temp = tester.widget<CheckboxListTile>(
        find.ancestor(
          of: find.text('Temperature'),
          matching: find.byType(CheckboxListTile),
        ),
      );
      expect(temp.value, isTrue);
      final snow = tester.widget<CheckboxListTile>(
        find.ancestor(
          of: find.text('Snow'),
          matching: find.byType(CheckboxListTile),
        ),
      );
      expect(snow.value, isFalse);
    });

    testWidgets('tapping a metric toggles the visible checkbox', (tester) async {
      await pumpSheet(tester);
      // Snow starts unchecked.
      CheckboxListTile snowTile() => tester.widget<CheckboxListTile>(
        find.ancestor(
          of: find.text('Snow'),
          matching: find.byType(CheckboxListTile),
        ),
      );
      expect(snowTile().value, isFalse);
      await tester.tap(find.text('Snow'));
      await tester.pumpAndSettle();
      expect(snowTile().value, isTrue);
    });

    // NOTE: a "Save persists" test that exercises the full Save → DB roundtrip
    // is intentionally left to `marker_weather_prefs_notifier_test.dart`
    // which calls `setMetrics(...)` directly. Driving Save through tester.tap
    // here trips an event-loop hang against the in-memory DB factory swap that
    // setUp performs — a known sqflite-ffi quirk during widget tests — without
    // adding behavioral coverage beyond the notifier test.
  });
}
