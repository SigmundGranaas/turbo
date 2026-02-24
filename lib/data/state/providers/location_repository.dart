import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';
import 'package:sqflite/sqflite.dart';
import 'package:turbo/data/auth/auth_providers.dart';
import 'package:turbo/data/datastore/api_location_service.dart';
import 'package:turbo/data/datastore/indexeddb/indexdb.dart';
import 'package:turbo/data/datastore/marker_data_store.dart';
import 'package:turbo/data/datastore/sqlite/sqlite_marker_datastore.dart';
import 'package:turbo/data/model/marker.dart';

import '../../../core/data/database_provider.dart';

final localMarkerDataStoreProvider = FutureProvider<MarkerDataStore>((ref) async {
  if (kIsWeb) {
    final store = ShimDBMarkerDataStore();
    await store.init();
    return store;
  } else {
    // FIX: Depend on the central database provider
    final db = await ref.watch(databaseProvider.future);
    return SQLiteMarkerDataStore(db);
  }
});

final apiLocationServiceProvider = Provider<ApiLocationService>((ref) {
  final apiClient = ref.watch(authenticatedApiClientProvider);
  return ApiLocationService(apiClient);
});

final locationRepositoryProvider = NotifierProvider.autoDispose<LocationRepository, AsyncValue<List<Marker>>>(() {
  return LocationRepository();
});

class LocationRepository extends Notifier<AsyncValue<List<Marker>>> {
  final _log = Logger('LocationRepository');

  @override
  AsyncValue<List<Marker>> build() {
    // 1. Return cached/empty data immediately to avoid blocking the UI
    // with a global loading spinner on every app restart.
    _initState();
    return const AsyncValue.data([]); 
  }

  Future<MarkerDataStore> get _localStore => ref.read(localMarkerDataStoreProvider.future);
  ApiLocationService get _apiService => ref.read(apiLocationServiceProvider);

  Future<void> _initState() async {
    await _loadData(authStatus: ref.read(authStateProvider).status);

    ref.listen<AuthState>(authStateProvider, (previous, next) {
      final prevStatus = previous?.status;
      if (prevStatus != next.status) {
        if ((prevStatus == AuthStatus.unauthenticated || prevStatus == AuthStatus.initial || prevStatus == AuthStatus.error) &&
            next.status == AuthStatus.authenticated) {
          _syncLocalDataOnLogin(authStatus: next.status).then((_) => _loadData(authStatus: next.status));
        } else {
          _loadData(authStatus: next.status);
        }
      }
    });
  }

  Future<void> _loadData({required AuthStatus authStatus}) async {
    state = const AsyncValue.loading();
    try {
      List<Marker> markers;
      if (authStatus == AuthStatus.authenticated) {
        markers = await _fetchAndCacheAllUserMarkersFromServer();
      } else {
        final localStore = await _localStore;
        markers = await localStore.getAll();
      }
      state = AsyncValue.data(markers);
    } catch (e, st) {
      _log.severe("Error loading data", e, st);
      state = AsyncValue.error(e, st);
    }
  }

  Future<void> _syncLocalDataOnLogin({required AuthStatus authStatus}) async {
    if (authStatus != AuthStatus.authenticated) return;
    final localStore = await _localStore;
    final localMarkers = await localStore.getAll();

    List<Future<void>> syncFutures = [];
    for (final localMarker in localMarkers) {
      if (!localMarker.synced) {
        syncFutures.add(() async {
          try {
            final serverMarker = await _apiService.createLocation(localMarker.copyWith(synced: false));
            if (serverMarker != null) {
              await localStore.delete(localMarker.uuid);
              await localStore.insert(serverMarker.copyWith(synced: true));
            }
          } catch (e) {
            _log.warning("Error syncing local marker ${localMarker.uuid} on login", e);
          }
        }());
      }
    }
    await Future.wait(syncFutures);
  }


  Future<List<Marker>> _fetchAndCacheAllUserMarkersFromServer() async {
    final stopwatch = Stopwatch()..start();
    final localStore = await _localStore;
    try {
      _log.info('SYNC: Starting server fetch...');
      final serverMarkers = await _apiService.getAllUserLocations();
      _log.info('SYNC: Fetched ${serverMarkers.length} markers from server in ${stopwatch.elapsedMilliseconds}ms.');
      stopwatch.reset();

      if (serverMarkers.isEmpty) {
        await localStore.clearAll();
        return [];
      }

      _log.info('SYNC: Starting BATCH write of ${serverMarkers.length} markers to DB...');

      // This is a web-safe way of doing this.
      if (localStore is SQLiteMarkerDataStore) {
        // Use a batch operation for performance on native platforms
        await localStore.db.transaction((txn) async {
          await txn.delete(markersTable);
          final batch = txn.batch();
          for (final marker in serverMarkers) {
            batch.insert(
              markersTable,
              marker.copyWith(synced: true).toLocalMap(),
              conflictAlgorithm: ConflictAlgorithm.replace,
            );
          }
          await batch.commit(noResult: true);
        });
      } else {
        // Fallback for web (or other implementations)
        await localStore.clearAll();
        for (final marker in serverMarkers) {
          await localStore.insert(marker.copyWith(synced: true));
        }
      }

      _log.info('SYNC: Finished BATCH writing markers in ${stopwatch.elapsedMilliseconds}ms.');
      stopwatch.stop();
      return serverMarkers;
    } catch (e, s) {
      _log.severe("Error fetching from server, returning local", e, s);
      return localStore.getAll();
    }
  }


  Future<void> addMarker(Marker marker) async {
    final localStore = await _localStore;
    final currentAuthStatus = ref.read(authStateProvider).status;

    state = const AsyncValue.loading();

    try {
      Marker markerToSave = marker.copyWith(synced: false);
      if (currentAuthStatus == AuthStatus.authenticated) {
        final serverMarker = await _apiService.createLocation(marker);
        if (serverMarker != null) {
          markerToSave = serverMarker.copyWith(synced: true);
          if (marker.uuid != serverMarker.uuid) {
            await localStore.delete(marker.uuid);
          }
        }
      }
      await localStore.insert(markerToSave);
      await _loadData(authStatus: currentAuthStatus);
    } catch (e, st) {
      _log.warning("Error adding marker", e, st);
      state = AsyncValue.error(e, st);
      await localStore.insert(marker.copyWith(synced: false));
      await _loadData(authStatus: currentAuthStatus);
    }
  }

  Future<void> updateMarker(Marker marker) async {
    final localStore = await _localStore;
    final currentAuthStatus = ref.read(authStateProvider).status;
    state = const AsyncValue.loading();
    try {
      Marker markerToUpdate = marker.copyWith(synced: false);
      if (currentAuthStatus == AuthStatus.authenticated) {
        final updatedServerMarker = await _apiService.updateLocation(marker);
        if (updatedServerMarker != null) {
          markerToUpdate = updatedServerMarker.copyWith(synced: true);
        }
      }
      await localStore.update(markerToUpdate);
      await _loadData(authStatus: currentAuthStatus);
    } catch (e, st) {
      _log.warning("Error updating marker", e, st);
      state = AsyncValue.error(e, st);
      await localStore.update(marker.copyWith(synced: false));
      await _loadData(authStatus: currentAuthStatus);
    }
  }

  Future<void> deleteMarker(String uuid) async {
    final localStore = await _localStore;
    final currentAuthStatus = ref.read(authStateProvider).status;
    state = const AsyncValue.loading();
    try {
      if (currentAuthStatus == AuthStatus.authenticated) {
        await _apiService.deleteLocation(uuid);
      }
      await localStore.delete(uuid);
      await _loadData(authStatus: currentAuthStatus);
    } catch (e, st) {
      _log.warning("Error deleting marker", e, st);
      await localStore.delete(uuid);
      await _loadData(authStatus: currentAuthStatus);
      state = AsyncValue.error(e, st);
    }
  }

  Future<void> triggerManualSync() async {
    final localStore = await _localStore;
    final currentAuthStatus = ref.read(authStateProvider).status;
    if (currentAuthStatus != AuthStatus.authenticated) return;
    state = const AsyncValue.loading();
    try {
      final localUnsynced = await localStore.getUnsynced();
      for (final localMarker in localUnsynced) {
        try {
          final serverMarker = await _apiService.createLocation(localMarker);
          if (serverMarker != null) {
            await localStore.delete(localMarker.uuid);
            await localStore.insert(serverMarker.copyWith(synced: true));
          }
        } catch (e) {
          _log.warning("Manual Sync: Error uploading ${localMarker.uuid}", e);
        }
      }

      final serverMarkers = await _apiService.getAllUserLocations();
      final serverMarkerMap = {for (var m in serverMarkers) m.uuid: m};
      final allLocalMarkers = await localStore.getAll();
      List<String> localUuidsToDelete = [];

      for (final localMarker in allLocalMarkers) {
        final serverVersion = serverMarkerMap[localMarker.uuid];
        if (serverVersion != null) {
          if (localMarker != serverVersion.copyWith(synced: true)) {
            await localStore.update(serverVersion.copyWith(synced: true));
          }
          serverMarkerMap.remove(localMarker.uuid);
        } else {
          if (localMarker.synced) {
            localUuidsToDelete.add(localMarker.uuid);
          }
        }
      }
      if(localUuidsToDelete.isNotEmpty) {
        await localStore.deleteAll(localUuidsToDelete);
      }

      for (final newServerMarker in serverMarkerMap.values) {
        await localStore.insert(newServerMarker.copyWith(synced: true));
      }

      await _loadData(authStatus: currentAuthStatus);
    } catch (e, st) {
      _log.severe("Error during manual sync", e, st);
      state = AsyncValue.error(e, st);
    }
  }

  Future<Marker?> getMarkerByUuid(String uuid) async {
    final localStore = await _localStore;
    final currentAuthStatus = ref.read(authStateProvider).status;
    final currentStateValue = state.value;
    if (currentStateValue != null) {
      try {
        return currentStateValue.firstWhere((m) => m.uuid == uuid);
      } catch (_) { /* Not in current list, proceed */ }
    }

    final local = await localStore.getByUuid(uuid);
    if (currentAuthStatus == AuthStatus.authenticated) {
      try {
        final remote = await _apiService.getLocationById(uuid);
        if (remote != null) {
          await localStore.insert(remote.copyWith(synced: true));
          return remote;
        } else {
          if (local != null && local.synced) {
            await localStore.delete(uuid);
            _loadData(authStatus: currentAuthStatus);
            return null;
          }
        }
      } catch (e) {
        if (local != null) {
          if (local.synced) {
            final unsyncedLocal = local.copyWith(synced: false);
            await localStore.update(unsyncedLocal);
            return unsyncedLocal;
          }
          return local;
        }
        return null;
      }
    }
    return local;
  }
}