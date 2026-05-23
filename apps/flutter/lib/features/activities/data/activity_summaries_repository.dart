import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';
import 'package:turbo/core/connectivity/connectivity_provider.dart';
import 'package:turbo/features/auth/api.dart';

import '../models/activity_summary.dart';
import 'activity_offline_stores.dart';
import 'activity_summaries_api.dart';
import 'activity_summary_store.dart';

final activitySummariesApiProvider = Provider<ActivitySummariesApi>((ref) {
  final apiClient = ref.watch(authenticatedApiClientProvider);
  return ActivitySummariesApi(apiClient);
});

/// Holds the union of all summaries the client has heard about from the
/// server. Driven by delta-sync: the in-memory map keeps the latest known
/// state per id, tombstones remove. Per-kind detail data lives in its own
/// kind feature's repository.
///
/// Offline behavior: on init we load whatever the SQLite store has so the
/// map paints pins on cold start before any network round-trip. Every
/// delta-sync result is persisted; tombstones remove rows locally too.
/// A connectivity listener triggers `refresh()` when the network returns
/// from offline so a long disconnect catches up automatically.
final activitySummariesRepositoryProvider =
    NotifierProvider<ActivitySummariesRepository, AsyncValue<Map<String, ActivitySummary>>>(() {
  return ActivitySummariesRepository();
});

class ActivitySummariesRepository
    extends Notifier<AsyncValue<Map<String, ActivitySummary>>> {
  final _log = Logger('ActivitySummariesRepository');
  DateTime? _cursor;

  @override
  AsyncValue<Map<String, ActivitySummary>> build() {
    // Don't block the UI; the bootstrap path loads the local cache first
    // (so the map paints immediately) and then hits the server in the
    // background for the delta.
    Future.microtask(_bootstrap);

    ref.listen<bool>(connectivityProvider, (prev, next) {
      // When connectivity returns from offline, pull whatever we missed.
      if (prev == false && next == true) {
        unawaited(refresh());
      }
    });

    // Privacy: when the signed-in user changes (logout or switch),
    // drop the previous user's pins immediately and reset the delta
    // cursor + the local sqlite cache so the new user starts from a
    // clean slate. Without this, the in-memory map carries the
    // previous user's summaries into the next session, and the next
    // cold-start bootstrap re-hydrates them from disk — making the
    // previous user's pins briefly visible on a different user's map.
    ref.listen<AuthStatus>(
      authStateProvider.select((s) => s.status),
      (prev, next) {
        if (prev == next) return;
        if (prev == AuthStatus.authenticated &&
            next != AuthStatus.authenticated) {
          state = const AsyncValue.data({});
          _cursor = null;
          unawaited(_wipeCaches());
        } else if (next == AuthStatus.authenticated &&
            prev != AuthStatus.authenticated) {
          // New session — reset cursor and re-bootstrap so the new
          // user's locally-cached and server-side data drive the map.
          state = const AsyncValue.data({});
          _cursor = null;
          unawaited(_bootstrap());
        }
      },
    );

    return const AsyncValue.data({});
  }

  ActivitySummariesApi get _api => ref.read(activitySummariesApiProvider);
  Future<ActivitySummaryStore> get _store =>
      ref.read(activitySummaryStoreProvider.future);

  Future<void> _bootstrap() async {
    // Step 1: hydrate from local cache so the map has pins right away.
    try {
      final store = await _store;
      final cached = await store.getAll();
      if (cached.isNotEmpty) {
        state = AsyncValue.data({for (final s in cached) s.id: s});
        // Use the most recent updated_at as our delta cursor seed so the
        // first refresh asks the server only for what's newer. Cheap and
        // close enough — the server's delta endpoint is idempotent on
        // overlap.
        _cursor = cached
            .map((s) => s.updatedAt)
            .reduce((a, b) => a.isAfter(b) ? a : b);
      }
    } catch (e, st) {
      _log.warning('Failed to hydrate summaries from local store', e, st);
    }

    // Step 2: pull the delta in the background.
    await refresh();
  }

  /// Pull changes since the last cursor. Safe to call repeatedly.
  Future<void> refresh() async {
    try {
      final delta = await _api.getChanges(since: _cursor);
      final current = Map<String, ActivitySummary>.from(state.value ?? {});
      for (final item in delta.items) {
        current[item.id] = item;
      }
      for (final t in delta.deleted) {
        current.remove(t.id);
      }
      _cursor = delta.serverTime;
      state = AsyncValue.data(current);

      // Persist deltas so the next cold start has them.
      unawaited(_persistDelta(delta.items, delta.deleted.map((t) => t.id).toList()));
    } catch (e, st) {
      _log.warning('Failed to refresh activity summaries', e, st);
      // Keep previous data on transient failures; surface error only if we
      // have nothing yet.
      if (state.value == null || (state.value?.isEmpty ?? true)) {
        state = AsyncValue.error(e, st);
      }
    }
  }

  Future<void> _persistDelta(
      List<ActivitySummary> upserts, List<String> deletes) async {
    try {
      final store = await _store;
      if (upserts.isNotEmpty) await store.upsertMany(upserts);
      if (deletes.isNotEmpty) await store.deleteMany(deletes);
    } catch (e, st) {
      _log.warning('Failed to persist summaries delta', e, st);
    }
    // Tombstoned activities should also evict their per-kind caches so
    // a stale detail/conditions payload doesn't get served after delete.
    for (final id in deletes) {
      unawaited(_evictKindCaches(id));
    }
  }

  Future<void> _evictKindCaches(String id) async {
    // We don't know the kind from a tombstone alone (the in-memory map
    // may already have been mutated), so evict across every known kind.
    // The store interface is cheap enough that this is fine — a single
    // PK lookup that's a no-op when the row doesn't exist.
    const knownKinds = [
      'fishing',
      'backcountry_ski',
      'hiking',
      'xc_ski',
      'packrafting',
      'freediving',
    ];
    try {
      final conditions = await ref.read(conditionsCacheStoreProvider.future);
      final details = await ref.read(activityDetailsCacheStoreProvider.future);
      for (final k in knownKinds) {
        await conditions.remove(activityId: id, kind: k);
        await details.remove(activityId: id, kind: k);
      }
    } catch (e, st) {
      _log.warning('Failed to evict caches for $id', e, st);
    }
  }

  /// Wipes every per-user activity cache. Called when the signed-in
  /// user changes so the next session never sees the previous user's
  /// data — either on a cold start (cache hydration) or on the first
  /// in-flight render after logout.
  Future<void> _wipeCaches() async {
    try {
      await (await _store).clearAll();
    } catch (e, st) {
      _log.warning('Failed to wipe activity summaries store', e, st);
    }
    try {
      final conditions = await ref.read(conditionsCacheStoreProvider.future);
      await conditions.clearAll();
    } catch (e, st) {
      _log.warning('Failed to wipe conditions cache', e, st);
    }
    try {
      final details = await ref.read(activityDetailsCacheStoreProvider.future);
      await details.clearAll();
    } catch (e, st) {
      _log.warning('Failed to wipe details cache', e, st);
    }
  }

  /// Insert/update one summary locally without going to the server.
  /// Called by a kind's repository immediately after a successful
  /// create/update so the map updates without waiting for the next delta
  /// sync round-trip.
  void upsertLocal(ActivitySummary summary) {
    final current = Map<String, ActivitySummary>.from(state.value ?? {});
    current[summary.id] = summary;
    state = AsyncValue.data(current);
    unawaited(_persistOne(summary));
  }

  /// Remove one summary locally.
  void removeLocal(String id) {
    final current = Map<String, ActivitySummary>.from(state.value ?? {});
    current.remove(id);
    state = AsyncValue.data(current);
    unawaited(_removeOne(id));
  }

  Future<void> _persistOne(ActivitySummary s) async {
    try {
      (await _store).upsert(s);
    } catch (e, st) {
      _log.warning('Failed to persist summary ${s.id}', e, st);
    }
  }

  Future<void> _removeOne(String id) async {
    try {
      (await _store).remove(id);
    } catch (e, st) {
      _log.warning('Failed to remove summary $id from local store', e, st);
    }
    unawaited(_evictKindCaches(id));
  }
}
