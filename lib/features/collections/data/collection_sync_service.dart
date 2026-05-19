import 'package:flutter/foundation.dart';

import '../models/collection.dart';
import '../models/collection_item_ref.dart';
import 'api_collection_service.dart';
import 'collection_data_store.dart';

/// High-water-mark cursor store for the collections delta-sync flow.
/// Identical contract to the tracks / locations cursor stores so the
/// same SharedPreferences/IndexedDB backing implementation can be reused.
abstract class CollectionSyncCursorStore {
  Future<DateTime?> getLastServerTime();
  Future<void> setLastServerTime(DateTime serverTime);
}

/// Orchestrates pull-then-push between the local collections store and
/// the Collections API. Mirrors TrackSyncService — the same outcomes and
/// the same server-wins conflict semantics on the initial roll-out.
///
/// Item membership is synced as a flat set: the local store's items are
/// reconciled against the server's items for each collection. Items the
/// server has and the local store doesn't get added locally; items the
/// local store has and the server doesn't get pushed; orphaned local
/// items (added while offline) are pushed; orphaned server items are
/// pulled.
class CollectionSyncService {
  final ApiCollectionService _api;
  final CollectionDataStore _store;
  final CollectionSyncCursorStore _cursor;

  CollectionSyncService({
    required ApiCollectionService api,
    required CollectionDataStore store,
    required CollectionSyncCursorStore cursor,
  })  : _api = api,
        _store = store,
        _cursor = cursor;

  Future<CollectionSyncOutcome> sync() async {
    final since = await _cursor.getLastServerTime();
    CollectionDeltaResult result;
    try {
      result = await _api.getCollectionsChangedSince(since: since);
    } catch (e, st) {
      if (kDebugMode) {
        // ignore: avoid_print
        print('CollectionSyncService: delta pull failed: $e\n$st');
      }
      return CollectionSyncOutcome.failed(e);
    }

    final allLocal = await _store.getAll();
    final localByUuid = {for (final c in allLocal) c.uuid: c};

    // Apply tombstones — server wins on delete intent.
    for (final tombstone in result.deleted) {
      if (localByUuid.containsKey(tombstone.uuid)) {
        await _store.delete(tombstone.uuid);
      }
    }

    // Pull: apply server changes locally.
    for (final remote in result.items) {
      final local = localByUuid[remote.collection.uuid];
      if (local == null) {
        await _store.insert(remote.collection);
        await _reconcileItems(remote.collection.uuid, const [], remote.items);
        continue;
      }
      // Prefer server copy when the local row was last synced; let the
      // unsynced local copy win until it's pushed. This is the same
      // server-wins-after-push rule as TrackSyncService.
      if (local.synced) {
        await _store.update(remote.collection);
        final localItems = await _store.getItems(local.uuid);
        await _reconcileItems(local.uuid, localItems, remote.items);
      } else if (remote.collection.updatedAt != null &&
          (local.updatedAt == null ||
              remote.collection.updatedAt!.isAfter(local.updatedAt!))) {
        await _store.update(remote.collection);
        final localItems = await _store.getItems(local.uuid);
        await _reconcileItems(local.uuid, localItems, remote.items);
      }
    }

    // Push: upload local unsynced creates/edits.
    final pushed = <String>[];
    for (final c in allLocal.where((c) => !c.synced)) {
      try {
        CollectionWithItems? response;
        if (c.version == null) {
          response = await _api.createCollection(c);
        } else {
          response = await _api.updateCollection(c);
        }
        if (response != null) {
          await _store.update(response.collection);
          final localItems = await _store.getItems(c.uuid);
          // After the metadata create/update, push local membership too.
          await _pushItemDelta(c.uuid, localItems, response.items, response.collection.version);
          pushed.add(c.uuid);
        }
      } on CollectionConflictException catch (conflict) {
        // Server wins. Overwrite the local copy with whatever the
        // server returned alongside the 412.
        if (conflict.current != null) {
          await _store.update(conflict.current!.collection);
          final localItems = await _store.getItems(c.uuid);
          await _reconcileItems(c.uuid, localItems, conflict.current!.items);
        }
      } catch (e, st) {
        if (kDebugMode) {
          // ignore: avoid_print
          print('CollectionSyncService: push failed for ${c.uuid}: $e\n$st');
        }
      }
    }

    await _cursor.setLastServerTime(result.serverTime);
    return CollectionSyncOutcome.success(
      pulled: result.items.length,
      pushed: pushed.length,
      deleted: result.deleted.length,
      serverTime: result.serverTime,
    );
  }

  /// Brings the local items table for [collectionUuid] into agreement
  /// with [remote]. Pull side: don't push anything, just align the local
  /// store with what the server says.
  Future<void> _reconcileItems(
    String collectionUuid,
    List<CollectionItemRef> local,
    List<CollectionItemRef> remote,
  ) async {
    final localSet = local.toSet();
    final remoteSet = remote.toSet();

    for (final toAdd in remoteSet.difference(localSet)) {
      await _store.addItem(collectionUuid, toAdd);
    }
    for (final toRemove in localSet.difference(remoteSet)) {
      await _store.removeItem(collectionUuid, toRemove);
    }
  }

  /// Push side: send each local-only item to the server and remove each
  /// server-only item the local store has dropped. Item-level conflicts
  /// (404 / 412) are best-effort — the next delta pull will reconcile.
  Future<void> _pushItemDelta(
    String collectionUuid,
    List<CollectionItemRef> local,
    List<CollectionItemRef> remote,
    int? version,
  ) async {
    final localSet = local.toSet();
    final remoteSet = remote.toSet();

    var currentVersion = version;
    for (final toAdd in localSet.difference(remoteSet)) {
      try {
        await _api.addItem(collectionUuid, toAdd, ifMatchVersion: currentVersion);
        // The server's version bumps by one on each successful item
        // mutation; keep the cursor advancing so the next If-Match
        // doesn't 412 ourselves.
        if (currentVersion != null) currentVersion = currentVersion + 1;
      } on CollectionConflictException {
        // Re-pull will reconcile.
        return;
      } catch (_) {
        // Best effort; abort the push and let the next sync round retry.
        return;
      }
    }
    for (final toRemove in remoteSet.difference(localSet)) {
      try {
        await _api.removeItem(collectionUuid, toRemove,
            ifMatchVersion: currentVersion);
        if (currentVersion != null) currentVersion = currentVersion + 1;
      } on CollectionConflictException {
        return;
      } catch (_) {
        return;
      }
    }
  }
}

class CollectionSyncOutcome {
  final bool ok;
  final Object? error;
  final int pulled;
  final int pushed;
  final int deleted;
  final DateTime? serverTime;

  const CollectionSyncOutcome._({
    required this.ok,
    required this.error,
    required this.pulled,
    required this.pushed,
    required this.deleted,
    required this.serverTime,
  });

  factory CollectionSyncOutcome.success({
    required int pulled,
    required int pushed,
    required int deleted,
    required DateTime serverTime,
  }) =>
      CollectionSyncOutcome._(
        ok: true,
        error: null,
        pulled: pulled,
        pushed: pushed,
        deleted: deleted,
        serverTime: serverTime,
      );

  factory CollectionSyncOutcome.failed(Object error) =>
      CollectionSyncOutcome._(
        ok: false,
        error: error,
        pulled: 0,
        pushed: 0,
        deleted: 0,
        serverTime: null,
      );
}
