import 'package:logging/logging.dart';

import 'api_track_service.dart';
import 'saved_path_data_store.dart';

final _log = Logger('TrackSyncService');

/// Holds the high-water-mark `serverTime` cursor between calls. The
/// store is platform-agnostic: SharedPreferences on mobile/desktop,
/// IndexedDB on web. We pass it in via the abstract interface so the
/// orchestration logic itself stays storage-free.
abstract class TrackSyncCursorStore {
  Future<DateTime?> getLastServerTime();
  Future<void> setLastServerTime(DateTime serverTime);
}

/// Orchestrates pull-then-push between the local store and the Tracks
/// API. Mirrors what [LocationRepository] does for markers, but uses
/// the delta-sync model instead of bbox fetches + a synced bool.
///
/// Call [sync] on login + on connectivity-restored. Tests can drive
/// the orchestrator directly.
class TrackSyncService {
  final ApiTrackService _api;
  final SavedPathDataStore _store;
  final TrackSyncCursorStore _cursor;

  TrackSyncService({
    required ApiTrackService api,
    required SavedPathDataStore store,
    required TrackSyncCursorStore cursor,
  })  : _api = api,
        _store = store,
        _cursor = cursor;

  /// Single round-trip: pull-since-cursor, then push local pending
  /// writes (unsynced creates + edits) and deletes (placeholder for the
  /// pending_track_deletes table when the data store grows hooks for it).
  Future<TrackSyncOutcome> sync() async {
    final since = await _cursor.getLastServerTime();
    TrackDeltaResult result;
    try {
      result = await _api.getTracksChangedSince(since: since);
    } catch (e, st) {
      _log.warning('delta pull failed', e, st);
      return TrackSyncOutcome.failed(e);
    }

    // Apply server changes locally. Tombstones win over unsynced local
    // edits — the server is the source of truth for delete intent.
    final allPaths = await _store.getAll();
    final localByUuid = {for (final p in allPaths) p.uuid: p};

    for (final tombstone in result.deleted) {
      if (localByUuid.containsKey(tombstone.uuid)) {
        await _store.delete(tombstone.uuid);
      }
    }

    for (final remote in result.items) {
      final local = localByUuid[remote.uuid];
      if (local == null) {
        await _store.insert(remote);
        continue;
      }
      // Prefer the server copy unless we have an unsynced local edit
      // strictly newer than the server's UpdatedAt. The simple version
      // of "server wins" is correct for the initial roll-out — see the
      // plan for conflict-merge follow-up.
      if (local.synced) {
        await _store.update(remote);
      } else if (remote.updatedAt != null &&
          (local.updatedAt == null || remote.updatedAt!.isAfter(local.updatedAt!))) {
        await _store.update(remote);
      }
    }

    // Push local unsynced creates/edits. Anything that hasn't been
    // committed to the server yet (synced=false) gets uploaded.
    final pushed = <String>[];
    for (final p in allPaths.where((p) => !p.synced)) {
      try {
        final response = p.version == null
            ? await _api.createTrack(p)
            : await _api.updateTrack(p);
        if (response != null) {
          await _store.update(response);
          pushed.add(p.uuid);
        }
      } on TrackConflictException catch (conflict) {
        // Server wins. Pull the current copy and overwrite the local
        // unsynced one. The UI can later surface a merge prompt; the
        // initial impl avoids ever leaving the device's data ahead of
        // the server.
        if (conflict.current != null) {
          await _store.update(conflict.current!);
        }
      } catch (e, st) {
        _log.warning('push failed for ${p.uuid}', e, st);
      }
    }

    await _cursor.setLastServerTime(result.serverTime);
    return TrackSyncOutcome.success(
      pulled: result.items.length,
      pushed: pushed.length,
      deleted: result.deleted.length,
      serverTime: result.serverTime,
    );
  }
}

class TrackSyncOutcome {
  final bool ok;
  final Object? error;
  final int pulled;
  final int pushed;
  final int deleted;
  final DateTime? serverTime;

  const TrackSyncOutcome._({
    required this.ok,
    required this.error,
    required this.pulled,
    required this.pushed,
    required this.deleted,
    required this.serverTime,
  });

  factory TrackSyncOutcome.success({
    required int pulled,
    required int pushed,
    required int deleted,
    required DateTime serverTime,
  }) =>
      TrackSyncOutcome._(
        ok: true,
        error: null,
        pulled: pulled,
        pushed: pushed,
        deleted: deleted,
        serverTime: serverTime,
      );

  factory TrackSyncOutcome.failed(Object error) => TrackSyncOutcome._(
        ok: false,
        error: error,
        pulled: 0,
        pushed: 0,
        deleted: 0,
        serverTime: null,
      );
}
