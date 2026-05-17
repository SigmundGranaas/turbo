import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/data/database_provider.dart';

import '../models/marker_weather_prefs.dart';
import '../models/weather_metric.dart';
import 'indexdb_marker_weather_prefs_store.dart';
import 'marker_weather_prefs_store.dart';
import 'sqlite_marker_weather_prefs_store.dart';

/// Platform-routed store: SQLite on native, IndexedDB on web.
final markerWeatherPrefsStoreProvider =
    FutureProvider<MarkerWeatherPrefsStore>((ref) async {
  if (kIsWeb) {
    return IndexedDBMarkerWeatherPrefsStore();
  }
  final db = await ref.watch(databaseProvider.future);
  return SQLiteMarkerWeatherPrefsStore(db);
});

/// Per-marker weather preferences. Family keyed by marker UUID.
///
/// Returns [MarkerWeatherPrefs.defaultMetrics] synchronously on build so the
/// UI never blocks on an initial read; the real row (if any) is loaded in the
/// background and the state is updated as soon as it arrives. Writes go
/// through [setMetrics], which is optimistic — state moves first, then the
/// persistence call.
final markerWeatherPrefsProvider = NotifierProvider.family<
    MarkerWeatherPrefsNotifier, MarkerWeatherPrefs, String>(
  MarkerWeatherPrefsNotifier.new,
);

class MarkerWeatherPrefsNotifier extends Notifier<MarkerWeatherPrefs> {
  MarkerWeatherPrefsNotifier(this.markerUuid);

  final String markerUuid;

  @override
  MarkerWeatherPrefs build() {
    _load();
    return MarkerWeatherPrefs.defaults(markerUuid);
  }

  Future<void> _load() async {
    final store = await ref.read(markerWeatherPrefsStoreProvider.future);
    final loaded = await store.get(markerUuid);
    if (loaded != null) {
      state = loaded;
    }
  }

  Future<void> setMetrics(Set<WeatherMetric> metrics) async {
    final updated = state.copyWith(metrics: metrics);
    state = updated;
    final store = await ref.read(markerWeatherPrefsStoreProvider.future);
    await store.upsert(updated);
  }
}
