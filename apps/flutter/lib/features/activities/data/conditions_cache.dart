import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';
import 'package:turbo/core/api/api_client.dart';
import 'package:turbo/features/auth/api.dart';

import 'activity_offline_stores.dart';
import 'conditions_cache_store.dart';

final _log = Logger('ActivityCache');

/// Raw conditions HTTP client. Kind-agnostic — callers supply the URL slug
/// (e.g. "backcountry-ski", "fishing"). Returns the unparsed JSON map so
/// callers can store it verbatim in [ConditionsCacheStore] and re-parse on
/// retrieval with their own typed `fromJson`.
final conditionsApiProvider = Provider<ConditionsApi>((ref) {
  final client = ref.watch(authenticatedApiClientProvider);
  return ConditionsApi(client);
});

class ConditionsApi {
  final ApiClient _client;
  ConditionsApi(this._client);

  Future<Map<String, dynamic>> getConditionsJson({
    required String kindUrlSlug,
    required String activityId,
    DateTime? at,
  }) async {
    final query = <String, dynamic>{};
    if (at != null) query['at'] = at.toUtc().toIso8601String();
    final r = await _client.get(
      '/api/activities/$kindUrlSlug/$activityId/conditions',
      queryParameters: query,
    );
    if (r.statusCode != 200) {
      throw Exception(
          'Failed to fetch conditions for $activityId: ${r.statusCode}');
    }
    return r.data as Map<String, dynamic>;
  }

  Future<Map<String, dynamic>> getActivityJson({
    required String kindUrlSlug,
    required String activityId,
  }) async {
    final r = await _client.get(
      '/api/activities/$kindUrlSlug/$activityId',
    );
    if (r.statusCode != 200) {
      throw Exception('Failed to fetch $kindUrlSlug $activityId: ${r.statusCode}');
    }
    return r.data as Map<String, dynamic>;
  }

  /// `GET /api/activities/{kind}/{id}/analysis` — the v2 orchestrator
  /// endpoint. Returns the unparsed JSON map so the caller can stash it
  /// in the analysis cache and re-parse on read.
  Future<Map<String, dynamic>> getAnalysisJson({
    required String kindUrlSlug,
    required String activityId,
    DateTime? at,
  }) async {
    final query = <String, dynamic>{};
    if (at != null) query['at'] = at.toUtc().toIso8601String();
    final r = await _client.get(
      '/api/activities/$kindUrlSlug/$activityId/analysis',
      queryParameters: query,
    );
    if (r.statusCode != 200) {
      throw Exception(
          'Failed to fetch analysis for $activityId: ${r.statusCode}');
    }
    return r.data as Map<String, dynamic>;
  }
}

/// Cache-aware conditions fetch. Hits the network first; on success
/// persists the raw JSON to the cache so the next offline open still
/// renders. On failure (timeout, offline, 5xx) falls back to the cached
/// payload if one exists and decodes it through the supplied parser.
///
/// The kind-specific provider supplies the URL slug (with hyphens —
/// `backcountry-ski`), the storage kind key (with underscores —
/// `backcountry_ski`, matches the server's kind taxonomy), and the
/// typed `fromJson`. Behaviour:
///
///   * fresh fetch ok    → return fresh, cache updated
///   * fetch fails, hit  → return cached (typed report retains its own
///                         `fetchedAt`, so the UI can show how stale it is)
///   * fetch fails, miss → rethrow the original error
Future<T> fetchConditionsCached<T>({
  required Ref ref,
  required String kindUrlSlug,
  required String kindKey,
  required String activityId,
  required T Function(Map<String, dynamic>) fromJson,
}) async {
  final cache = await ref.read(conditionsCacheStoreProvider.future);
  return _fetchWithCache<T>(
    ref: ref,
    cache: cache,
    cacheLabel: 'conditions',
    kindKey: kindKey,
    activityId: activityId,
    fromJson: fromJson,
    fetchRaw: () => ref.read(conditionsApiProvider).getConditionsJson(
          kindUrlSlug: kindUrlSlug,
          activityId: activityId,
        ),
  );
}

/// Cache-aware activity-detail fetch. Same semantics as
/// [fetchConditionsCached] but for `/api/activities/{kind}/{id}`. Lets
/// the detail screens render last-known state when a user taps a pin
/// while offline.
Future<T> fetchActivityCached<T>({
  required Ref ref,
  required String kindUrlSlug,
  required String kindKey,
  required String activityId,
  required T Function(Map<String, dynamic>) fromJson,
}) async {
  final cache = await ref.read(activityDetailsCacheStoreProvider.future);
  return _fetchWithCache<T>(
    ref: ref,
    cache: cache,
    cacheLabel: 'detail',
    kindKey: kindKey,
    activityId: activityId,
    fromJson: fromJson,
    fetchRaw: () => ref.read(conditionsApiProvider).getActivityJson(
          kindUrlSlug: kindUrlSlug,
          activityId: activityId,
        ),
  );
}

/// Cache-aware analysis fetch — the v2 orchestrator counterpart of
/// [fetchConditionsCached]. Hits the new `/analysis` endpoint, persists
/// the raw JSON in the analysis cache, falls back to the cached payload
/// on network failure so the detail screen still renders meaningful drivers
/// and warnings when offline (with the `Provenance.fetchedAt` value
/// visible so the user knows it's not current).
Future<T> fetchAnalysisCached<T>({
  required Ref ref,
  required String kindUrlSlug,
  required String kindKey,
  required String activityId,
  required T Function(Map<String, dynamic>) fromJson,
}) async {
  final cache = await ref.read(activityAnalysisCacheStoreProvider.future);
  return _fetchWithCache<T>(
    ref: ref,
    cache: cache,
    cacheLabel: 'analysis',
    kindKey: kindKey,
    activityId: activityId,
    fromJson: fromJson,
    fetchRaw: () => ref.read(conditionsApiProvider).getAnalysisJson(
          kindUrlSlug: kindUrlSlug,
          activityId: activityId,
        ),
  );
}

Future<T> _fetchWithCache<T>({
  required Ref ref,
  required PayloadCacheStore cache,
  required String cacheLabel,
  required String kindKey,
  required String activityId,
  required T Function(Map<String, dynamic>) fromJson,
  required Future<Map<String, dynamic>> Function() fetchRaw,
}) async {
  try {
    final raw = await fetchRaw();
    try {
      await cache.put(
        activityId: activityId,
        kind: kindKey,
        payloadJson: jsonEncode(raw),
      );
    } catch (e, st) {
      _log.warning('Failed to write $cacheLabel cache for $kindKey/$activityId', e, st);
    }
    return fromJson(raw);
  } catch (fetchError) {
    try {
      final hit = await cache.get(activityId: activityId, kind: kindKey);
      if (hit != null) {
        _log.info('Serving cached $cacheLabel for $kindKey/$activityId '
            '(fetched ${hit.fetchedAt.toIso8601String()})');
        return fromJson(jsonDecode(hit.payloadJson) as Map<String, dynamic>);
      }
    } catch (cacheError, cacheStack) {
      _log.warning('$cacheLabel cache read failed for $kindKey/$activityId',
          cacheError, cacheStack);
    }
    // No cache fallback — let the original upstream error propagate.
    rethrow;
  }
}
