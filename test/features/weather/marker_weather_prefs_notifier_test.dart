import 'package:flutter_riverpod/flutter_riverpod.dart';
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

ProviderContainer _makeContainer(MarkerWeatherPrefsStore store) {
  return ProviderContainer(overrides: [
    markerWeatherPrefsStoreProvider.overrideWith((ref) async => store),
  ]);
}

/// Wait briefly for the async post-build load to settle.
Future<void> _pump() => Future<void>.delayed(const Duration(milliseconds: 20));

void main() {
  group('MarkerWeatherPrefsNotifier', () {
    late Database db;
    late MarkerWeatherPrefsStore store;
    late ProviderContainer container;

    setUp(() async {
      db = await _openDb();
      store = SQLiteMarkerWeatherPrefsStore(db);
      container = _makeContainer(store);
    });

    tearDown(() async {
      container.dispose();
      await db.close();
    });

    test('build returns defaults synchronously for an unknown marker',
        () async {
      final prefs = container.read(markerWeatherPrefsProvider('new-uuid'));
      expect(prefs.markerUuid, 'new-uuid');
      expect(prefs.metrics, MarkerWeatherPrefs.defaultMetrics);
    });

    test('after async load, state reflects the stored row', () async {
      await store.upsert(MarkerWeatherPrefs(
        markerUuid: 'aaa',
        metrics: const {WeatherMetric.snow, WeatherMetric.waveHeight},
      ));

      // Initial read returns defaults (the seed).
      container.read(markerWeatherPrefsProvider('aaa'));
      await _pump();
      final loaded = container.read(markerWeatherPrefsProvider('aaa'));
      expect(loaded.metrics, {WeatherMetric.snow, WeatherMetric.waveHeight});
    });

    test('setMetrics persists and updates state', () async {
      container.read(markerWeatherPrefsProvider('aaa'));
      await _pump();
      await container
          .read(markerWeatherPrefsProvider('aaa').notifier)
          .setMetrics({WeatherMetric.uvIndex, WeatherMetric.waveHeight});

      final state = container.read(markerWeatherPrefsProvider('aaa'));
      expect(state.metrics, {WeatherMetric.uvIndex, WeatherMetric.waveHeight});

      final persisted = await store.get('aaa');
      expect(persisted!.metrics,
          {WeatherMetric.uvIndex, WeatherMetric.waveHeight});
    });

    test('different uuids have independent state', () async {
      await container
          .read(markerWeatherPrefsProvider('aaa').notifier)
          .setMetrics({WeatherMetric.snow});
      await container
          .read(markerWeatherPrefsProvider('bbb').notifier)
          .setMetrics({WeatherMetric.waveHeight});

      expect(container.read(markerWeatherPrefsProvider('aaa')).metrics,
          {WeatherMetric.snow});
      expect(container.read(markerWeatherPrefsProvider('bbb')).metrics,
          {WeatherMetric.waveHeight});
    });
  });
}
