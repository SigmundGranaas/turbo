import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/features/weather/api.dart';
import 'package:turbo/features/weather/data/sqlite_marker_weather_prefs_store.dart';

import '../../helpers/pump_app.dart' show initSqfliteFfi;

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
  group('SQLiteMarkerWeatherPrefsStore', () {
    late Database db;
    late MarkerWeatherPrefsStore store;

    setUp(() async {
      db = await _openDb();
      store = SQLiteMarkerWeatherPrefsStore(db);
    });

    tearDown(() async {
      await db.close();
    });

    test('get returns null for an unknown uuid', () async {
      expect(await store.get('not-stored'), isNull);
    });

    test('upsert then get round-trips the metric set', () async {
      final prefs = MarkerWeatherPrefs(
        markerUuid: 'aaa',
        metrics: const {
          WeatherMetric.temperature,
          WeatherMetric.waveHeight,
          WeatherMetric.snow,
        },
      );
      await store.upsert(prefs);

      final loaded = await store.get('aaa');
      expect(loaded, isNotNull);
      expect(loaded!.markerUuid, 'aaa');
      expect(loaded.metrics, prefs.metrics);
    });

    test('upsert replaces an existing row', () async {
      await store.upsert(MarkerWeatherPrefs(
        markerUuid: 'aaa',
        metrics: const {WeatherMetric.temperature},
      ));
      await store.upsert(MarkerWeatherPrefs(
        markerUuid: 'aaa',
        metrics: const {WeatherMetric.wind, WeatherMetric.snow},
      ));
      final loaded = await store.get('aaa');
      expect(loaded!.metrics, {WeatherMetric.wind, WeatherMetric.snow});
    });

    test('delete removes the row', () async {
      await store.upsert(MarkerWeatherPrefs(
        markerUuid: 'aaa',
        metrics: const {WeatherMetric.temperature},
      ));
      await store.delete('aaa');
      expect(await store.get('aaa'), isNull);
    });

    test('delete of an unknown uuid is a no-op', () async {
      await store.delete('does-not-exist');
      expect(await store.get('does-not-exist'), isNull);
    });
  });
}
