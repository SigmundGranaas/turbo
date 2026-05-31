import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/data/database_provider.dart';

import '../models/activity_summary.dart';
import 'activity_summary_store.dart';
import 'conditions_cache_store.dart';

/// SQLite-backed activity summary store on mobile/desktop; an in-memory
/// no-op on web (web doesn't share the cold-start offline scenario —
/// IndexedDB would be the right answer there, parallel to how
/// markers does it, but the existing in-memory fallback is non-broken).
final activitySummaryStoreProvider =
    FutureProvider<ActivitySummaryStore>((ref) async {
  if (kIsWeb) return _NoopActivitySummaryStore();
  final db = await ref.watch(databaseProvider.future);
  return SqliteActivitySummaryStore(db);
});

/// Local cache for per-(activity, kind) conditions reports. Same web /
/// non-web split as the summary store above.
final conditionsCacheStoreProvider =
    FutureProvider<ConditionsCacheStore>((ref) async {
  if (kIsWeb) return _NoopPayloadCacheStore();
  final db = await ref.watch(databaseProvider.future);
  return SqlitePayloadCacheStore.conditions(db);
});

/// Local cache for per-(activity, kind) detail payloads (the body of
/// `/api/activities/{kind}/{id}`). Same web / non-web split.
final activityDetailsCacheStoreProvider =
    FutureProvider<ActivityDetailsCacheStore>((ref) async {
  if (kIsWeb) return _NoopPayloadCacheStore();
  final db = await ref.watch(databaseProvider.future);
  return SqlitePayloadCacheStore.details(db);
});

/// Local cache for per-(activity, kind) `ActivityAnalysis` payloads. Same
/// web / non-web split as the other stores. Kept separate from the
/// conditions cache so the legacy report shape and the richer analysis
/// shape can coexist while kinds migrate.
final activityAnalysisCacheStoreProvider =
    FutureProvider<ActivityAnalysisCacheStore>((ref) async {
  if (kIsWeb) return _NoopPayloadCacheStore();
  final db = await ref.watch(databaseProvider.future);
  return SqlitePayloadCacheStore.analysis(db);
});

class _NoopActivitySummaryStore implements ActivitySummaryStore {
  @override
  Future<List<ActivitySummary>> getAll() async => const [];
  @override
  Future<void> upsertMany(List<ActivitySummary> items) async {}
  @override
  Future<void> upsert(ActivitySummary item) async {}
  @override
  Future<void> deleteMany(List<String> ids) async {}
  @override
  Future<void> remove(String id) async {}
  @override
  Future<void> clearAll() async {}
}

class _NoopPayloadCacheStore implements PayloadCacheStore {
  @override
  Future<CachedPayload?> get({required String activityId, required String kind}) async => null;
  @override
  Future<void> put({
    required String activityId,
    required String kind,
    required String payloadJson,
    DateTime? fetchedAt,
  }) async {}
  @override
  Future<void> remove({required String activityId, required String kind}) async {}
  @override
  Future<void> clearAll() async {}
}
