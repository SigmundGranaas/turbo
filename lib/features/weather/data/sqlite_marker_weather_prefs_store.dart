import 'dart:convert';

import 'package:sqflite/sqflite.dart';

import '../models/marker_weather_prefs.dart';
import 'marker_weather_prefs_store.dart';

class SQLiteMarkerWeatherPrefsStore implements MarkerWeatherPrefsStore {
  final Database db;
  SQLiteMarkerWeatherPrefsStore(this.db);

  @override
  Future<MarkerWeatherPrefs?> get(String markerUuid) async {
    final rows = await db.query(
      markerWeatherPrefsTable,
      where: 'marker_uuid = ?',
      whereArgs: [markerUuid],
      limit: 1,
    );
    if (rows.isEmpty) return null;
    final metricsJson =
        jsonDecode(rows.first['metrics'] as String) as Map<String, dynamic>?;
    if (metricsJson == null) {
      return MarkerWeatherPrefs.defaults(markerUuid);
    }
    return MarkerWeatherPrefs.fromJson(markerUuid, metricsJson);
  }

  @override
  Future<void> upsert(MarkerWeatherPrefs prefs) async {
    await db.insert(
      markerWeatherPrefsTable,
      {
        'marker_uuid': prefs.markerUuid,
        'metrics': jsonEncode(prefs.toJson()),
      },
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  @override
  Future<void> delete(String markerUuid) async {
    await db.delete(
      markerWeatherPrefsTable,
      where: 'marker_uuid = ?',
      whereArgs: [markerUuid],
    );
  }
}
