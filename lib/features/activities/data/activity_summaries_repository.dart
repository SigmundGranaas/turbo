import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';
import 'package:turbo/features/auth/api.dart';

import '../models/activity_summary.dart';
import 'activity_summaries_api.dart';

final activitySummariesApiProvider = Provider<ActivitySummariesApi>((ref) {
  final apiClient = ref.watch(authenticatedApiClientProvider);
  return ActivitySummariesApi(apiClient);
});

/// Holds the union of all summaries the client has heard about from the
/// server. Driven by delta-sync: the in-memory map keeps the latest known
/// state per id, tombstones remove. Per-kind detail data lives in its own
/// kind feature's repository.
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
    // Don't block the UI; let `refresh()` do its work in the background.
    Future.microtask(refresh);
    return const AsyncValue.data({});
  }

  ActivitySummariesApi get _api => ref.read(activitySummariesApiProvider);

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
    } catch (e, st) {
      _log.warning('Failed to refresh activity summaries', e, st);
      // Keep previous data on transient failures; surface error only if we
      // have nothing yet.
      if (state.value == null) {
        state = AsyncValue.error(e, st);
      }
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
  }

  /// Remove one summary locally.
  void removeLocal(String id) {
    final current = Map<String, ActivitySummary>.from(state.value ?? {});
    current.remove(id);
    state = AsyncValue.data(current);
  }
}
