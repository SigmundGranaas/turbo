import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/auth/auth_providers.dart';
import 'package:turbo/data/datastore/api_location_service.dart';
import 'package:turbo/data/datastore/indexeddb/indexdb.dart';
import 'package:turbo/data/datastore/marker_data_store.dart';
import 'package:turbo/data/datastore/sqlite/sqlite_marker_datastore.dart';
import 'package:turbo/data/model/marker.dart';

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

  Future<void> _initState() async {
    await _localStore.init();
    await _loadData(authStatus: _ref.read(authStateProvider).status);

    final listener = _ref.listen<AuthState>(authStateProvider, (previous, next) {
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

    if (!kIsWeb) {
      _authSubscription = listener as StreamSubscription?;
    }
  }

  Future<void> _loadData({required AuthStatus authStatus}) async {
    if (!mounted) return;
    state = const AsyncValue.loading();
    try {
      List<Marker> markers;
      if (authStatus == AuthStatus.authenticated) {
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

  Future<void> _syncLocalDataOnLogin({required AuthStatus authStatus}) async {
    if (authStatus != AuthStatus.authenticated) return;
    final localMarkers = await _localStore.getAll();

    List<Future<void>> syncFutures = [];
    for (final localMarker in localMarkers) {
      if (!localMarker.synced) {
        syncFutures.add(() async {
          try {
            final serverMarker = await _apiService.createLocation(localMarker.copyWith(synced: false));
            if (serverMarker != null) {
              await _localStore.delete(localMarker.uuid);
              await _localStore.insert(serverMarker.copyWith(synced: true));
            }
          } catch (e) {
            if (kDebugMode) print("Error syncing local marker ${localMarker.uuid} on login: $e");
          }
        }());
      }
    }
    await Future.wait(syncFutures);
  }


  Future<List<Marker>> _fetchAndCacheAllUserMarkersFromServer() async {
    try {
      final serverMarkers = await _apiService.getAllUserLocations();
      await _localStore.clearAll();
      for (final marker in serverMarkers) {
        await _localStore.insert(marker.copyWith(synced: true));
      }
      return serverMarkers;
    } catch (e) {
      if (kDebugMode) print("Error fetching from server, returning local: $e");
      return _localStore.getAll();
    }
  }

  Future<void> addMarker(Marker marker) async {
    if (!mounted) return;
    final currentAuthStatus = _ref.read(authStateProvider).status;

    state = const AsyncValue.loading();

    try {
      Marker markerToSave = marker.copyWith(synced: false);
      if (currentAuthStatus == AuthStatus.authenticated) {
        final serverMarker = await _apiService.createLocation(marker);
        if (serverMarker != null) {
          markerToSave = serverMarker.copyWith(synced: true);
          if (marker.uuid != serverMarker.uuid) {
            await _localStore.delete(marker.uuid);
          }
        }
      }
      await _localStore.insert(markerToSave);
      await _loadData(authStatus: currentAuthStatus);
    } catch (e, st) {
      if (kDebugMode) print("Error adding marker: $e");
      if (mounted) state = AsyncValue.error(e, st);
      await _localStore.insert(marker.copyWith(synced: false));
      await _loadData(authStatus: currentAuthStatus);
    }
  }

  Future<void> updateMarker(Marker marker) async {
    if (!mounted) return;
    final currentAuthStatus = _ref.read(authStateProvider).status;
    state = const AsyncValue.loading();
    try {
      Marker markerToUpdate = marker.copyWith(synced: false);
      if (currentAuthStatus == AuthStatus.authenticated) {
        final updatedServerMarker = await _apiService.updateLocation(marker);
        if (updatedServerMarker != null) {
          markerToUpdate = updatedServerMarker.copyWith(synced: true);
        }
      }
      await _localStore.update(markerToUpdate);
      await _loadData(authStatus: currentAuthStatus);
    } catch (e, st) {
      if (kDebugMode) print("Error updating marker: $e");
      if (mounted) state = AsyncValue.error(e, st);
      await _localStore.update(marker.copyWith(synced: false));
      await _loadData(authStatus: currentAuthStatus);
    }
  }

  Future<void> deleteMarker(String uuid) async {
    if (!mounted) return;
    final currentAuthStatus = _ref.read(authStateProvider).status;
    state = const AsyncValue.loading();
    try {
      if (currentAuthStatus == AuthStatus.authenticated) {
        await _apiService.deleteLocation(uuid);
      }
      await _localStore.delete(uuid);
      await _loadData(authStatus: currentAuthStatus);
    } catch (e, st) {
      if (kDebugMode) print("Error deleting marker: $e");
      await _localStore.delete(uuid);
      await _loadData(authStatus: currentAuthStatus);
      if (mounted) state = AsyncValue.error(e, st);
    }
  }

  Future<void> triggerManualSync() async {
    final currentAuthStatus = _ref.read(authStateProvider).status;
    if (currentAuthStatus != AuthStatus.authenticated || !mounted) return;
    state = const AsyncValue.loading();
    try {
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

      final serverMarkers = await _apiService.getAllUserLocations();
      final serverMarkerMap = {for (var m in serverMarkers) m.uuid: m};
      final allLocalMarkers = await _localStore.getAll();
      List<String> localUuidsToDelete = [];

      for (final localMarker in allLocalMarkers) {
        final serverVersion = serverMarkerMap[localMarker.uuid];
        if (serverVersion != null) {
          if (localMarker != serverVersion.copyWith(synced: true)) {
            await _localStore.update(serverVersion.copyWith(synced: true));
          }
          serverMarkerMap.remove(localMarker.uuid);
        } else {
          if (localMarker.synced) {
            localUuidsToDelete.add(localMarker.uuid);
          }
        }
      }
      if(localUuidsToDelete.isNotEmpty) {
        await _localStore.deleteAll(localUuidsToDelete);
      }

      for (final newServerMarker in serverMarkerMap.values) {
        await _localStore.insert(newServerMarker.copyWith(synced: true));
      }

      await _loadData(authStatus: currentAuthStatus);
    } catch (e, st) {
      if (kDebugMode) print("Error during manual sync: $e");
      if (mounted) state = AsyncValue.error(e, st);
    }
  }

  Future<Marker?> getMarkerByUuid(String uuid) async {
    final currentAuthStatus = _ref.read(authStateProvider).status;
    final currentStateValue = state.asData?.value;
    if (currentStateValue != null) {
      try {
        return currentStateValue.firstWhere((m) => m.uuid == uuid);
      } catch (_) { /* Not in current list, proceed */ }
    }

    final local = await _localStore.getByUuid(uuid);
    if (currentAuthStatus == AuthStatus.authenticated) {
      try {
        final remote = await _apiService.getLocationById(uuid);
        if (remote != null) {
          await _localStore.insert(remote.copyWith(synced: true));
          return remote;
        } else {
          if (local != null && local.synced) {
            await _localStore.delete(uuid);
            _loadData(authStatus: currentAuthStatus);
            return null;
          }
        }
      } catch (e) {
        if (local != null) {
          if (local.synced) {
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