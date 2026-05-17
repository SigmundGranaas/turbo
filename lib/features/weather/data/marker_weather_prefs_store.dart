import '../models/marker_weather_prefs.dart';

/// Persists per-marker weather preferences. Local-only — these preferences
/// don't sync to the server (by user decision).
abstract class MarkerWeatherPrefsStore {
  Future<MarkerWeatherPrefs?> get(String markerUuid);
  Future<void> upsert(MarkerWeatherPrefs prefs);
  Future<void> delete(String markerUuid);
}

/// SQL/IndexedDB store name shared across implementations.
///
/// Mirrors `markerWeatherPrefsTableName` in
/// `lib/core/data/database_provider.dart` — keeping them in sync is checked
/// at startup when the DB opens.
const String markerWeatherPrefsTable = 'marker_weather_prefs';
