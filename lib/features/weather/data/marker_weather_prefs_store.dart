import '../models/marker_weather_prefs.dart';

/// Persists per-marker weather preferences. Local-only — these preferences
/// don't sync to the server (by user decision).
abstract class MarkerWeatherPrefsStore {
  Future<MarkerWeatherPrefs?> get(String markerUuid);
  Future<void> upsert(MarkerWeatherPrefs prefs);
  Future<void> delete(String markerUuid);
}

/// SQL/IndexedDB table name shared across implementations.
const String markerWeatherPrefsTable = 'marker_weather_prefs';
