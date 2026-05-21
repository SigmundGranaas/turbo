import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/core/api/api_client.dart';
import 'package:turbo/core/connectivity/connectivity_provider.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/activities/data/activity_summaries_api.dart';
import 'package:turbo/features/activities/data/activity_summaries_repository.dart';
import 'package:turbo/features/activities/data/conditions_cache.dart';
import 'package:turbo/features/auth/api.dart';

import '../../helpers/in_memory_db.dart';
import '../../helpers/wait_for.dart';

// ---------------------------------------------------------------------------
// Fakes
//
// The activities shell talks to the server through two providers we can
// override at the seam:
//   * `activitySummariesApiProvider`  — cross-kind delta/bbox endpoints
//   * `conditionsApiProvider`         — per-kind conditions + detail GETs
// Both fakes operate on in-memory state so tests can drive every branch
// without booting an HTTP layer.
// ---------------------------------------------------------------------------

class FakeActivitySummariesApi extends ActivitySummariesApi {
  final List<activities.ActivitySummary> serverItems = [];
  final List<activities.ActivitySummaryTombstone> serverDeletes = [];
  bool shouldFail = false;
  int getChangesCalls = 0;

  FakeActivitySummariesApi() : super(ApiClient(baseUrl: 'http://test'));

  @override
  Future<ActivitySummariesDelta> getChanges({DateTime? since, int? limit}) async {
    getChangesCalls++;
    if (shouldFail) throw Exception('Network error');
    final filteredItems = since == null
        ? List.of(serverItems)
        : serverItems.where((s) => s.updatedAt.isAfter(since)).toList();
    final filteredDeletes = since == null
        ? List.of(serverDeletes)
        : serverDeletes.where((t) => t.deletedAt.isAfter(since)).toList();
    return ActivitySummariesDelta(
      items: filteredItems,
      deleted: filteredDeletes,
      serverTime: DateTime.now().toUtc(),
    );
  }

  @override
  Future<ActivitySummariesResponse> getByBbox({
    required double minLon,
    required double minLat,
    required double maxLon,
    required double maxLat,
    List<String>? kinds,
  }) async {
    if (shouldFail) throw Exception('Network error');
    return ActivitySummariesResponse(
      items: List.of(serverItems),
      serverTime: DateTime.now().toUtc(),
    );
  }
}

class FakeConditionsApi extends ConditionsApi {
  /// Keyed by `${kindUrlSlug}|${activityId}` → JSON map the API will return.
  final Map<String, Map<String, dynamic>> conditionsServer = {};
  final Map<String, Map<String, dynamic>> detailsServer = {};
  bool shouldFail = false;
  int getConditionsCalls = 0;
  int getActivityCalls = 0;

  FakeConditionsApi() : super(ApiClient(baseUrl: 'http://test'));

  @override
  Future<Map<String, dynamic>> getConditionsJson({
    required String kindUrlSlug,
    required String activityId,
    DateTime? at,
  }) async {
    getConditionsCalls++;
    if (shouldFail) throw Exception('Network error');
    final hit = conditionsServer['$kindUrlSlug|$activityId'];
    if (hit == null) throw Exception('404 — no conditions for $kindUrlSlug/$activityId');
    return hit;
  }

  @override
  Future<Map<String, dynamic>> getActivityJson({
    required String kindUrlSlug,
    required String activityId,
  }) async {
    getActivityCalls++;
    if (shouldFail) throw Exception('Network error');
    final hit = detailsServer['$kindUrlSlug|$activityId'];
    if (hit == null) throw Exception('404 — no activity for $kindUrlSlug/$activityId');
    return hit;
  }
}

class TestAuthNotifier extends AuthStateNotifier {
  @override
  AuthState build() => AuthState(status: AuthStatus.authenticated, email: 't@t.t');
}

class TestConnectivityNotifier extends ConnectivityNotifier {
  final bool _initial;
  TestConnectivityNotifier([this._initial = true]);

  @override
  bool build() => _initial;

  void setOnline() => state = true;
  void setOffline() => state = false;
}

// ---------------------------------------------------------------------------
// Test-only providers that exercise the activities-shell cache helpers.
// Each one passes through `fetchConditionsCached` / `fetchActivityCached` so
// the helper's behaviour is what the test asserts. The family parameter is
// a compound `kindUrlSlug|kindKey|activityId` so a single provider can drive
// every shape we need to verify.
// ---------------------------------------------------------------------------

({String urlSlug, String key, String id}) _splitArg(String arg) {
  final parts = arg.split('|');
  return (urlSlug: parts[0], key: parts[1], id: parts[2]);
}

final _testConditionsProvider =
    FutureProvider.family<Map<String, dynamic>, String>((ref, arg) async {
  final p = _splitArg(arg);
  return activities.fetchConditionsCached<Map<String, dynamic>>(
    ref: ref,
    kindUrlSlug: p.urlSlug,
    kindKey: p.key,
    activityId: p.id,
    fromJson: (json) => json,
  );
});

final _testActivityProvider =
    FutureProvider.family<Map<String, dynamic>, String>((ref, arg) async {
  final p = _splitArg(arg);
  return activities.fetchActivityCached<Map<String, dynamic>>(
    ref: ref,
    kindUrlSlug: p.urlSlug,
    kindKey: p.key,
    activityId: p.id,
    fromJson: (json) => json,
  );
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

activities.ActivitySummary _makeSummary({
  String? id,
  String kind = 'fishing',
  String name = 'Test spot',
  double lon = 10.0,
  double lat = 60.0,
  String iconKey = 'fishing',
  String? colorHex = '#1E6FB8',
  DateTime? updatedAt,
  int version = 1,
}) =>
    activities.ActivitySummary(
      id: id ?? 'id-${name.hashCode}',
      kind: kind,
      name: name,
      geometry: activities.ActivityGeometry.fromServer(
        wkt: 'POINT($lon $lat)',
        geometryKind: 'POINT',
      ),
      iconKey: iconKey,
      colorHex: colorHex,
      updatedAt: updatedAt ?? DateTime.now().toUtc(),
      version: version,
    );

activities.ActivitySummaryTombstone _makeTombstone(String id,
        {String kind = 'fishing', DateTime? deletedAt, int version = 2}) =>
    activities.ActivitySummaryTombstone(
      id: id,
      kind: kind,
      deletedAt: deletedAt ?? DateTime.now().toUtc(),
      version: version,
    );

Map<String, dynamic> _conditionsPayload({
  required String activityId,
  String rationale = 'Looks good',
}) =>
    {
      'activityId': activityId,
      'validAt': DateTime.now().toUtc().toIso8601String(),
      'fetchedAt': DateTime.now().toUtc().toIso8601String(),
      'weather': {
        'validAt': DateTime.now().toUtc().toIso8601String(),
        'airTemperatureCelsius': 12.0,
        'airPressureHpa': 1013.0,
        'relativeHumidityPct': 65.0,
        'cloudCoveragePct': 30.0,
        'windSpeedMs': 4.0,
        'windGustMs': null,
        'windFromDegrees': 180.0,
        'precipitationNext1hMm': null,
        'precipitationNext6hMm': null,
        'symbolCode': 'partlycloudy_day',
      },
      'score': 80,
      'rationale': rationale,
    };

Map<String, dynamic> _fishingDetailPayload({required String id, String name = 'Server name'}) => {
      'id': id,
      'ownerId': 'owner-1',
      'name': name,
      'description': null,
      'longitude': 10.0,
      'latitude': 60.0,
      'details': {
        'waterKind': 0,
        'shoreOrBoat': 0,
        'accessNotes': null,
        'targetSpecies': [],
        'knownDepths': [],
        'preferred': null,
      },
      'createdAt': DateTime.now().toUtc().toIso8601String(),
      'updatedAt': DateTime.now().toUtc().toIso8601String(),
      'version': 1,
    };

Future<Map<String, activities.ActivitySummary>> _waitForSummaries(
        ProviderContainer container) =>
    waitForAsyncData(container, activitySummariesRepositoryProvider);

Future<Map<String, dynamic>> _readConditions(
  ProviderContainer container,
  String urlSlug,
  String key,
  String id,
) {
  final arg = '$urlSlug|$key|$id';
  return waitForAsyncData(container, _testConditionsProvider(arg));
}

Future<Map<String, dynamic>> _readActivity(
  ProviderContainer container,
  String urlSlug,
  String key,
  String id,
) {
  final arg = '$urlSlug|$key|$id';
  return waitForAsyncData(container, _testActivityProvider(arg));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

void main() {
  late Database db;
  late ProviderContainer container;
  late FakeActivitySummariesApi fakeSummariesApi;
  late FakeConditionsApi fakeConditionsApi;
  late TestAuthNotifier testAuth;
  late TestConnectivityNotifier testConnectivity;

  Future<void> bootstrap({bool online = true, Database? preExistingDb}) async {
    db = preExistingDb ?? await createActivitiesDb();
    fakeSummariesApi = FakeActivitySummariesApi();
    fakeConditionsApi = FakeConditionsApi();
    testAuth = TestAuthNotifier();
    testConnectivity = TestConnectivityNotifier(online);

    container = ProviderContainer(overrides: [
      databaseProvider.overrideWith((ref) async => db),
      authStateProvider.overrideWith(() => testAuth),
      connectivityProvider.overrideWith(() => testConnectivity),
      activitySummariesApiProvider.overrideWithValue(fakeSummariesApi),
      conditionsApiProvider.overrideWithValue(fakeConditionsApi),
    ]);
    container.listen(activitySummariesRepositoryProvider, (_, _) {});
  }

  tearDown(() async {
    container.dispose();
    try {
      await db.close();
    } catch (_) {
      // Some tests close the DB themselves to exercise write-path errors.
    }
  });

  // -----------------------------------------------------------------------
  // 1. Summary persistence is the foundation of offline map rendering.
  // -----------------------------------------------------------------------
  group('Summaries persist across cold start', () {
    test('first refresh writes server items to SQLite', () async {
      await bootstrap();
      fakeSummariesApi.serverItems.addAll([
        _makeSummary(id: 'a-1', name: 'Pond'),
        _makeSummary(id: 'a-2', name: 'Lake'),
      ]);

      final summaries = await _waitForSummaries(container);
      expect(summaries.keys, containsAll(['a-1', 'a-2']));

      final rows = await db.query(activitySummariesTable);
      expect(rows.map((r) => r['id']), containsAll(['a-1', 'a-2']));
    });

    test('cold start hydrates from SQLite before the first network call',
        () async {
      // Seed the DB ahead of provider initialization.
      final seedDb = await createActivitiesDb();
      await seedDb.insert(activitySummariesTable, {
        'id': 'pre-existing',
        'kind': 'fishing',
        'name': 'Cached',
        'geometry_wkt': 'POINT(11 61)',
        'geometry_kind': 'POINT',
        'icon_key': 'fishing',
        'color_hex': '#1E6FB8',
        'updated_at': DateTime.now().toUtc().millisecondsSinceEpoch,
        'version': 1,
      });

      await bootstrap(preExistingDb: seedDb);
      // Block server refresh so we can prove the data came from SQLite.
      fakeSummariesApi.shouldFail = true;

      final summaries = await _waitForSummaries(container);
      expect(summaries['pre-existing'], isNotNull,
          reason: 'Cached row must be visible even when the server refresh fails.');
      expect(summaries['pre-existing']!.name, 'Cached');
    });

    test('tombstone in delta removes the row from SQLite', () async {
      await bootstrap();
      fakeSummariesApi.serverItems.add(_makeSummary(id: 'doomed', name: 'Will be deleted'));
      await _waitForSummaries(container);
      expect((await db.query(activitySummariesTable)).length, 1);

      fakeSummariesApi.serverItems.clear();
      fakeSummariesApi.serverDeletes.add(_makeTombstone('doomed'));
      await container.read(activitySummariesRepositoryProvider.notifier).refresh();
      await Future.delayed(const Duration(milliseconds: 50));

      expect(await db.query(activitySummariesTable), isEmpty,
          reason: 'Tombstoned row must be removed from SQLite.');
    });
  });

  // -----------------------------------------------------------------------
  // 2. Optimistic mutations mirror to SQLite.
  // -----------------------------------------------------------------------
  group('Optimistic mutations write through to SQLite', () {
    test('upsertLocal persists the row immediately', () async {
      await bootstrap();
      await _waitForSummaries(container);

      final s = _makeSummary(id: 'opt-1', name: 'Just added');
      container.read(activitySummariesRepositoryProvider.notifier).upsertLocal(s);
      await Future.delayed(const Duration(milliseconds: 100));

      final rows = await db.query(activitySummariesTable, where: 'id = ?', whereArgs: ['opt-1']);
      expect(rows, hasLength(1));
      expect(rows.first['name'], 'Just added');
    });

    test('removeLocal removes the row + evicts conditions/details caches',
        () async {
      await bootstrap();
      await db.insert(activitySummariesTable, {
        'id': 'to-evict',
        'kind': 'fishing',
        'name': 'Evict me',
        'geometry_wkt': 'POINT(10 60)',
        'geometry_kind': 'POINT',
        'icon_key': 'fishing',
        'color_hex': '#1E6FB8',
        'updated_at': DateTime.now().toUtc().millisecondsSinceEpoch,
        'version': 1,
      });
      await db.insert(activityConditionsCacheTable, {
        'activity_id': 'to-evict',
        'kind': 'fishing',
        'payload_json': '{}',
        'fetched_at': DateTime.now().toUtc().millisecondsSinceEpoch,
      });
      await db.insert(activityDetailsCacheTable, {
        'activity_id': 'to-evict',
        'kind': 'fishing',
        'payload_json': '{}',
        'fetched_at': DateTime.now().toUtc().millisecondsSinceEpoch,
      });

      await _waitForSummaries(container);
      container.read(activitySummariesRepositoryProvider.notifier).removeLocal('to-evict');
      await Future.delayed(const Duration(milliseconds: 100));

      expect(
          await db.query(activitySummariesTable, where: 'id = ?', whereArgs: ['to-evict']),
          isEmpty);
      expect(
          await db.query(activityConditionsCacheTable, where: 'activity_id = ?', whereArgs: ['to-evict']),
          isEmpty,
          reason: 'Stale conditions must not survive a delete.');
      expect(
          await db.query(activityDetailsCacheTable, where: 'activity_id = ?', whereArgs: ['to-evict']),
          isEmpty,
          reason: 'Stale detail payload must not survive a delete.');
    });

    test('server-side tombstone also evicts the per-kind caches', () async {
      await bootstrap();
      // Seed all three caches for this id.
      await db.insert(activitySummariesTable, {
        'id': 'server-tomb',
        'kind': 'fishing',
        'name': 'Soon gone',
        'geometry_wkt': 'POINT(10 60)',
        'geometry_kind': 'POINT',
        'icon_key': 'fishing',
        'color_hex': '#1E6FB8',
        'updated_at': DateTime.now().toUtc().millisecondsSinceEpoch,
        'version': 1,
      });
      await db.insert(activityConditionsCacheTable, {
        'activity_id': 'server-tomb',
        'kind': 'fishing',
        'payload_json': '{}',
        'fetched_at': DateTime.now().toUtc().millisecondsSinceEpoch,
      });

      await _waitForSummaries(container);

      // Server-side tombstone in next refresh.
      fakeSummariesApi.serverDeletes.add(_makeTombstone('server-tomb'));
      await container.read(activitySummariesRepositoryProvider.notifier).refresh();
      await Future.delayed(const Duration(milliseconds: 100));

      expect(
          await db.query(activityConditionsCacheTable,
              where: 'activity_id = ?', whereArgs: ['server-tomb']),
          isEmpty,
          reason: 'Server-side delete must evict the cached conditions too.');
    });
  });

  // -----------------------------------------------------------------------
  // 3. Network failure does not wipe local state.
  // -----------------------------------------------------------------------
  group('Network failure is non-destructive', () {
    test('refresh failure with existing data keeps the in-memory state',
        () async {
      await bootstrap();
      fakeSummariesApi.serverItems.add(_makeSummary(id: 'sticky', name: 'Sticky'));
      await _waitForSummaries(container);

      fakeSummariesApi.shouldFail = true;
      await container.read(activitySummariesRepositoryProvider.notifier).refresh();

      final state = container.read(activitySummariesRepositoryProvider);
      expect(state.hasValue, isTrue,
          reason: 'Repository must NOT enter error state when it already has data.');
      expect(state.value!['sticky'], isNotNull);
    });

    test('connectivity returning from offline triggers a delta refresh',
        () async {
      await bootstrap(online: false);
      await _waitForSummaries(container);
      final initialCalls = fakeSummariesApi.getChangesCalls;

      fakeSummariesApi.serverItems.add(_makeSummary(id: 'reconnect-1', name: 'After reconnect'));
      testConnectivity.setOnline();
      await Future.delayed(const Duration(milliseconds: 100));

      expect(fakeSummariesApi.getChangesCalls, greaterThan(initialCalls),
          reason: 'Returning online must call getChanges() again.');
      expect(container.read(activitySummariesRepositoryProvider).value!['reconnect-1'],
          isNotNull);
    });
  });

  // -----------------------------------------------------------------------
  // 4. Conditions cache fallback. Every kind's provider routes through
  //    fetchConditionsCached; we exercise the helper directly via a
  //    test-only provider so a single suite covers the contract.
  // -----------------------------------------------------------------------
  group('Conditions cache survives a network drop', () {
    test('successful fetch persists the payload to the cache', () async {
      await bootstrap();
      await _waitForSummaries(container);
      fakeConditionsApi.conditionsServer['fishing|act-1'] =
          _conditionsPayload(activityId: 'act-1', rationale: 'Sunny');

      final report = await _readConditions(container, 'fishing', 'fishing', 'act-1');
      expect(report['rationale'], 'Sunny');

      final rows = await db.query(activityConditionsCacheTable,
          where: 'activity_id = ?', whereArgs: ['act-1']);
      expect(rows, hasLength(1));
      final stored = jsonDecode(rows.first['payload_json'] as String) as Map<String, dynamic>;
      expect(stored['rationale'], 'Sunny');
    });

    test('network failure falls back to the cached payload', () async {
      await bootstrap();
      await _waitForSummaries(container);

      fakeConditionsApi.conditionsServer['fishing|act-1'] =
          _conditionsPayload(activityId: 'act-1', rationale: 'Original');
      await _readConditions(container, 'fishing', 'fishing', 'act-1');

      // Invalidate the test provider so the next read re-runs the helper
      // and re-hits the (now-failing) network.
      container.invalidate(_testConditionsProvider('fishing|fishing|act-1'));
      fakeConditionsApi.shouldFail = true;

      final result = await _readConditions(container, 'fishing', 'fishing', 'act-1');
      expect(result['rationale'], 'Original',
          reason: 'Offline fetch must serve the last-known-good cache entry.');
    });

    test('network failure with empty cache rethrows', () async {
      await bootstrap();
      await _waitForSummaries(container);
      fakeConditionsApi.shouldFail = true;

      await expectLater(
        _readConditions(container, 'fishing', 'fishing', 'act-never-seen'),
        throwsA(isA<Exception>()),
        reason: 'No fresh response and no cache: caller must see the error.',
      );
    });

    test('different kinds share the store without collision', () async {
      await bootstrap();
      await _waitForSummaries(container);

      // Same activity id reused across two kinds.
      fakeConditionsApi.conditionsServer['fishing|same-id'] =
          _conditionsPayload(activityId: 'same-id', rationale: 'Fishing rationale');
      fakeConditionsApi.conditionsServer['hiking|same-id'] =
          _conditionsPayload(activityId: 'same-id', rationale: 'Hiking rationale');

      await _readConditions(container, 'fishing', 'fishing', 'same-id');
      await _readConditions(container, 'hiking', 'hiking', 'same-id');

      // Take both kinds offline and re-fetch — each must get its own payload.
      container.invalidate(_testConditionsProvider('fishing|fishing|same-id'));
      container.invalidate(_testConditionsProvider('hiking|hiking|same-id'));
      fakeConditionsApi.shouldFail = true;

      final fishing = await _readConditions(container, 'fishing', 'fishing', 'same-id');
      final hiking = await _readConditions(container, 'hiking', 'hiking', 'same-id');
      expect(fishing['rationale'], 'Fishing rationale');
      expect(hiking['rationale'], 'Hiking rationale');
    });

    test('a stale cache from a network failure does not overwrite a later success',
        () async {
      // The user opens a panel offline (cache hit), then comes online and
      // refreshes. The fresh response must overwrite the stale cache.
      await bootstrap();
      await _waitForSummaries(container);

      fakeConditionsApi.conditionsServer['fishing|act-1'] =
          _conditionsPayload(activityId: 'act-1', rationale: 'v1');
      await _readConditions(container, 'fishing', 'fishing', 'act-1');

      // Server changes, app reads — cache must reflect the new state.
      container.invalidate(_testConditionsProvider('fishing|fishing|act-1'));
      fakeConditionsApi.conditionsServer['fishing|act-1'] =
          _conditionsPayload(activityId: 'act-1', rationale: 'v2');
      final fresh = await _readConditions(container, 'fishing', 'fishing', 'act-1');
      expect(fresh['rationale'], 'v2');

      // Take the network down — should now return v2, not v1.
      container.invalidate(_testConditionsProvider('fishing|fishing|act-1'));
      fakeConditionsApi.shouldFail = true;
      final cached = await _readConditions(container, 'fishing', 'fishing', 'act-1');
      expect(cached['rationale'], 'v2');
    });
  });

  // -----------------------------------------------------------------------
  // 5. Activity detail cache — symmetric to conditions.
  // -----------------------------------------------------------------------
  group('Activity detail cache survives a network drop', () {
    test('successful fetch persists the detail payload', () async {
      await bootstrap();
      await _waitForSummaries(container);
      fakeConditionsApi.detailsServer['fishing|d-1'] =
          _fishingDetailPayload(id: 'd-1', name: 'My pond');

      final detail = await _readActivity(container, 'fishing', 'fishing', 'd-1');
      expect(detail['name'], 'My pond');

      final rows = await db.query(activityDetailsCacheTable,
          where: 'activity_id = ?', whereArgs: ['d-1']);
      expect(rows, hasLength(1));
    });

    test('network failure falls back to the cached detail', () async {
      await bootstrap();
      await _waitForSummaries(container);
      fakeConditionsApi.detailsServer['fishing|d-1'] =
          _fishingDetailPayload(id: 'd-1', name: 'Original name');
      await _readActivity(container, 'fishing', 'fishing', 'd-1');

      container.invalidate(_testActivityProvider('fishing|fishing|d-1'));
      fakeConditionsApi.shouldFail = true;
      final detail = await _readActivity(container, 'fishing', 'fishing', 'd-1');
      expect(detail['name'], 'Original name',
          reason: 'Offline detail open must show last-known activity body.');
    });

    test('detail and conditions caches are independent for the same id',
        () async {
      // Asserts the separation of the two SQLite tables. A detail cache hit
      // must not surface as a conditions response (different shapes).
      await bootstrap();
      await _waitForSummaries(container);

      fakeConditionsApi.detailsServer['fishing|shared'] =
          _fishingDetailPayload(id: 'shared', name: 'Detail only');
      await _readActivity(container, 'fishing', 'fishing', 'shared');

      // No conditions seeded.
      container.invalidate(_testConditionsProvider('fishing|fishing|shared'));
      fakeConditionsApi.shouldFail = true;
      await expectLater(
        _readConditions(container, 'fishing', 'fishing', 'shared'),
        throwsA(isA<Exception>()),
        reason: 'A detail-cache hit must not satisfy a conditions request.',
      );
    });
  });
}
