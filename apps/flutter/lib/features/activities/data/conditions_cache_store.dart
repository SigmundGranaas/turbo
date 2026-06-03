import 'package:sqflite/sqflite.dart';
import 'package:turbo/core/data/database_provider.dart';

/// Local cache for per-(activity, kind) payloads — used for both
/// conditions reports and activity detail bodies. The schema is the same
/// for both: one row per (activity_id, kind) keyed pair, with the raw
/// JSON payload and a fetched-at timestamp.
///
/// Two named instances (see [activitySummaryStoreProvider] siblings):
///   * [ConditionsCacheStore] — /api/activities/{kind}/{id}/conditions
///   * [ActivityDetailsCacheStore] — /api/activities/{kind}/{id}
///
/// Used as a fallback when the network fails so the UI keeps rendering
/// the last-known-good payload with a timestamp the user can see
/// (every kind's report exposes `fetchedAt`; the detail screens render
/// the activity as it was last fetched).
abstract class PayloadCacheStore {
  Future<CachedPayload?> get({required String activityId, required String kind});
  Future<void> put({
    required String activityId,
    required String kind,
    required String payloadJson,
    DateTime? fetchedAt,
  });
  Future<void> remove({required String activityId, required String kind});
  Future<void> clearAll();
}

class CachedPayload {
  final String payloadJson;
  final DateTime fetchedAt;
  const CachedPayload({required this.payloadJson, required this.fetchedAt});
}

/// Conditions-report cache (last successful /conditions response).
typedef ConditionsCacheStore = PayloadCacheStore;

/// Activity detail cache (last successful /api/activities/{kind}/{id}).
typedef ActivityDetailsCacheStore = PayloadCacheStore;

/// Analysis cache (last successful /api/activities/{kind}/{id}/analysis).
/// Separate from the conditions cache so the legacy
/// `{Kind}ConditionsReport` payload and the richer `ActivityAnalysis`
/// payload can coexist while kinds migrate.
typedef ActivityAnalysisCacheStore = PayloadCacheStore;

class SqlitePayloadCacheStore implements PayloadCacheStore {
  final Database _db;
  final String _table;
  SqlitePayloadCacheStore(this._db, this._table);

  /// Factory for the conditions table.
  factory SqlitePayloadCacheStore.conditions(Database db) =>
      SqlitePayloadCacheStore(db, activityConditionsCacheTable);

  /// Factory for the activity-details table.
  factory SqlitePayloadCacheStore.details(Database db) =>
      SqlitePayloadCacheStore(db, activityDetailsCacheTable);

  /// Factory for the analysis table.
  factory SqlitePayloadCacheStore.analysis(Database db) =>
      SqlitePayloadCacheStore(db, activityAnalysisCacheTable);

  @override
  Future<CachedPayload?> get({required String activityId, required String kind}) async {
    final rows = await _db.query(
      _table,
      where: 'activity_id = ? AND kind = ?',
      whereArgs: [activityId, kind],
      limit: 1,
    );
    if (rows.isEmpty) return null;
    final row = rows.first;
    return CachedPayload(
      payloadJson: row['payload_json'] as String,
      fetchedAt: DateTime.fromMillisecondsSinceEpoch(
          row['fetched_at'] as int,
          isUtc: true),
    );
  }

  @override
  Future<void> put({
    required String activityId,
    required String kind,
    required String payloadJson,
    DateTime? fetchedAt,
  }) async {
    final ts = (fetchedAt ?? DateTime.now()).toUtc().millisecondsSinceEpoch;
    await _db.insert(
      _table,
      {
        'activity_id': activityId,
        'kind': kind,
        'payload_json': payloadJson,
        'fetched_at': ts,
      },
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  @override
  Future<void> remove({required String activityId, required String kind}) async {
    await _db.delete(
      _table,
      where: 'activity_id = ? AND kind = ?',
      whereArgs: [activityId, kind],
    );
  }

  @override
  Future<void> clearAll() async {
    await _db.delete(_table);
  }
}

