import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/data/auth/auth_providers.dart';
import 'package:map_app/data/datastore/api_location_service.dart';
import 'package:map_app/data/datastore/indexeddb/indexdb.dart';
import 'package:map_app/data/datastore/marker_data_store.dart';
import 'package:map_app/data/datastore/sqlite/sqlite_marker_datastore.dart';
import 'package:map_app/data/model/marker.dart';

final localMarkerDataStoreProvider = Provider<MarkerDataStore>((ref) {
  if (kIsWeb) {
    return ShimDBMarkerDataStore();
  } else {
    return SQLiteMarkerDataStore();
  }
});

final apiLocationServiceProvider = Provider<ApiLocationService>((ref) {
  final apiClient = ref.watch(authenticatedApiClientProvider);
  return ApiLocationService(apiClient);
});

final locationRepositoryProvider = StateNotifierProvider.autoDispose<LocationRepository, AsyncValue<List<Marker>>>((ref) {
  return LocationRepository(ref);
});

class LocationRepository extends StateNotifier<AsyncValue<List<Marker>>> {
  final Ref _ref;
  StreamSubscription? _authSubscription;

  LocationRepository(this._ref) : super(const AsyncValue.loading()) {
    _initState();
  }

  MarkerDataStore get _localStore => _ref.read(localMarkerDataStoreProvider);
  ApiLocationService get _apiService => _ref.read(apiLocationServiceProvider);
  AuthStatus get _authStatus => _ref.watch(authStateProvider).status;

  Future<void> _initState() async {
    await _localStore.init();
    await _loadData();

    // Don't cast the result to StreamSubscription
    _authSubscription = null; // Initialize with null

    final listener = _ref.listen<AuthState>(authStateProvider, (previous, next) {
      final prevStatus = previous?.status;
      if (prevStatus != next.status) {
        if ((prevStatus == AuthStatus.unauthenticated || prevStatus == AuthStatus.initial || prevStatus == AuthStatus.error) &&
            next.status == AuthStatus.authenticated) {
          _syncLocalDataOnLogin().then((_) => _loadData());
        } else {
          _loadData();
        }
      }
    });

    // If we're on a platform that supports StreamSubscription, try to cast
    if (!kIsWeb) {
      try {
        _authSubscription = listener as StreamSubscription?;
      } catch (e) {
        if (kDebugMode) {
          print("Warning: Could not cast listener to StreamSubscription: $e");
        }
      }
    }
  }

  Future<void> _loadData() async {
    if (!mounted) return;
    state = const AsyncValue.loading();
    try {
      List<Marker> markers;
      if (_authStatus == AuthStatus.authenticated) {
        markers = await _fetchAndCacheAllUserMarkersFromServer();
      } else {
        markers = await _localStore.getAll();
      }
      if (mounted) state = AsyncValue.data(markers);
    } catch (e, st) {
      if (kDebugMode) print("Error loading data: $e");
      if (mounted) state = AsyncValue.error(e, st);
    }
  }

  Future<void> _syncLocalDataOnLogin() async {
    if (_authStatus != AuthStatus.authenticated) return;
    final localMarkers = await _localStore.getAll(); // Get all, not just unsynced, to handle potential conflicts

    List<Future<void>> syncFutures = [];
    for (final localMarker in localMarkers) {
      if (!localMarker.synced) { // Only try to upload markers that were explicitly local/unsynced
        syncFutures.add(() async {
          try {
            final serverMarker = await _apiService.createLocation(localMarker.copyWith(synced: false)); // Ensure it's sent as new
            if (serverMarker != null) {
              await _localStore.delete(localMarker.uuid); // Delete old local
              await _localStore.insert(serverMarker.copyWith(synced: true)); // Insert new synced from server
            }
          } catch (e) {
            if (kDebugMode) print("Error syncing local marker ${localMarker.uuid} on login: $e");
            // Marker remains local and unsynced if API call fails
          }
        }());
      }
    }
    await Future.wait(syncFutures);
    // After attempting to sync up local changes, a full fetch from server ensures consistency.
    // _loadData() will do this if called after this method.
  }


  Future<List<Marker>> _fetchAndCacheAllUserMarkersFromServer() async {
    try {
      final serverMarkers = await _apiService.getAllUserLocations();
      // Simple strategy: clear local and replace with server data.
      // More complex merging could be done here if needed.
      await _localStore.clearAll();
      for (final marker in serverMarkers) {
        await _localStore.insert(marker.copyWith(synced: true));
      }
      return serverMarkers;
    } catch (e) {
      if (kDebugMode) print("Error fetching from server, returning local: $e");
      // Fallback to local data if server fetch fails
      return _localStore.getAll();
    }
  }

  Future<void> addMarker(Marker marker) async {
    if (!mounted) return;

    final previousState = state;
    state = const AsyncValue.loading(); // Indicate loading

    try {
      Marker markerToSave = marker.copyWith(synced: false); // Assume unsynced initially
      if (_authStatus == AuthStatus.authenticated) {
        final serverMarker = await _apiService.createLocation(marker);
        if (serverMarker != null) {
          markerToSave = serverMarker.copyWith(synced: true);
          // If server assigns a new UUID, we need to remove the old local one if it was optimistically added
          if (marker.uuid != serverMarker.uuid) {
            await _localStore.delete(marker.uuid);
          }
          await _localStore.insert(markerToSave);
        } else {
          // API failed, save locally as unsynced
          await _localStore.insert(markerToSave);
        }
      } else {
        // Unauthenticated, save locally as unsynced
        await _localStore.insert(markerToSave);
      }
      await _loadData(); // Refresh the full list
    } catch (e, st) {
      if (kDebugMode) print("Error adding marker: $e");
      // Revert to previous state on error or handle specific error display
      if (mounted) state = AsyncValue.error(e, st);
      // Ensure it's saved locally as unsynced even if API call within auth block failed
      await _localStore.insert(marker.copyWith(synced: false));
      await _loadData(); // Try to reload, might show the locally saved one
    }
  }

  Future<void> updateMarker(Marker marker) async {
    if (!mounted) return;
    state = const AsyncValue.loading();
    try {
      Marker markerToUpdate = marker.copyWith(synced: false); // Assume unsynced on update attempt
      if (_authStatus == AuthStatus.authenticated) {
        final updatedServerMarker = await _apiService.updateLocation(marker);
        if (updatedServerMarker != null) {
          markerToUpdate = updatedServerMarker.copyWith(synced: true);
          await _localStore.update(markerToUpdate);
        } else {
          // API failed (e.g. 404 or other), mark as unsynced and update locally
          await _localStore.update(markerToUpdate);
        }
      } else {
        // Unauthenticated, update locally as unsynced
        await _localStore.update(markerToUpdate);
      }
      await _loadData();
    } catch (e, st) {
      if (kDebugMode) print("Error updating marker: $e");
      if (mounted) state = AsyncValue.error(e, st);
      await _localStore.update(marker.copyWith(synced: false));
      await _loadData();
    }
  }

  Future<void> deleteMarker(String uuid) async {
    if (!mounted) return;
    state = const AsyncValue.loading();
    try {
      if (_authStatus == AuthStatus.authenticated) {
        await _apiService.deleteLocation(uuid);
        // If API fails (e.g. 404), it's fine, local delete will proceed.
      }
      await _localStore.delete(uuid);
      await _loadData();
    } catch (e, st) {
      if (kDebugMode) print("Error deleting marker: $e");
      // Ensure local delete even on API error during auth
      await _localStore.delete(uuid);
      await _loadData(); // Try to reload
      if (mounted) state = AsyncValue.error(e, st);
    }
  }

  Future<void> triggerManualSync() async {
    if (_authStatus != AuthStatus.authenticated || !mounted) return;
    state = const AsyncValue.loading();
    try {
      // 1. Upload local unsynced changes
      final localUnsynced = await _localStore.getUnsynced();
      for (final localMarker in localUnsynced) {
        try {
          final serverMarker = await _apiService.createLocation(localMarker);
          if (serverMarker != null) {
            await _localStore.delete(localMarker.uuid);
            await _localStore.insert(serverMarker.copyWith(synced: true));
          }
        } catch (e) {
          if (kDebugMode) print("Manual Sync: Error uploading ${localMarker.uuid}: $e");
        }
      }

      // 2. Fetch all from server and reconcile (server is source of truth for synced items)
      final serverMarkers = await _apiService.getAllUserLocations();
      final serverMarkerMap = {for (var m in serverMarkers) m.uuid: m};

      final allLocalMarkers = await _localStore.getAll();
      List<String> localUuidsToDelete = [];

      // Update local store with server data or mark for deletion if not on server but was synced
      for (final localMarker in allLocalMarkers) {
        final serverVersion = serverMarkerMap[localMarker.uuid];
        if (serverVersion != null) {
          // Exists on server, ensure local is identical and synced
          if (localMarker != serverVersion.copyWith(synced: true)) {
            await _localStore.update(serverVersion.copyWith(synced: true));
          }
          serverMarkerMap.remove(localMarker.uuid); // Handled
        } else {
          // Not on server
          if (localMarker.synced) {
            // Was synced, but server doesn't have it -> delete locally
            localUuidsToDelete.add(localMarker.uuid);
          }
          // If !localMarker.synced, it was handled by the upload step or will remain local.
        }
      }
      if(localUuidsToDelete.isNotEmpty) {
        await _localStore.deleteAll(localUuidsToDelete);
      }

      // Add new markers from server that were not local
      for (final newServerMarker in serverMarkerMap.values) {
        await _localStore.insert(newServerMarker.copyWith(synced: true));
      }

      await _loadData(); // Reload everything
    } catch (e, st) {
      if (kDebugMode) print("Error during manual sync: $e");
      if (mounted) state = AsyncValue.error(e, st);
    }
  }

  Future<Marker?> getMarkerByUuid(String uuid) async {
    final currentStateValue = state.asData?.value;
    if (currentStateValue != null) {
      try {
        return currentStateValue.firstWhere((m) => m.uuid == uuid);
      } catch (_) { /* Not in current list, proceed */ }
    }

    final local = await _localStore.getByUuid(uuid);
    if (_authStatus == AuthStatus.authenticated) {
      try {
        final remote = await _apiService.getLocationById(uuid);
        if (remote != null) {
          await _localStore.insert(remote.copyWith(synced: true)); // Cache/update
          return remote;
        } else { // Not on remote (404)
          if (local != null && local.synced) {
            await _localStore.delete(uuid); // Was synced, now gone from server
            _loadData(); // Refresh list
            return null;
          }
        }
      } catch (e) {
        // API error, return local if available, potentially marking as unsynced
        if (local != null) {
          if (local.synced) { // If it thought it was synced, mark as not authoritative now
            final unsyncedLocal = local.copyWith(synced: false);
            await _localStore.update(unsyncedLocal);
            return unsyncedLocal;
          }
          return local;
        }
        return null;
      }
    }
    return local;
  }

  @override
  void dispose() {
    _authSubscription?.cancel();
    super.dispose();
  }
}