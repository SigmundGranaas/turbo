import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import '../../auth/auth_providers.dart';
import '../../datastore/factory.dart';
import '../../model/marker.dart';

part 'location_provider.g.dart';

@riverpod
class LocationNotifier extends _$LocationNotifier {
  @override
  FutureOr<List<Marker>> build() async {
    // Listen to auth state changes
    ref.listen(authStateProvider, (previous, next) {
      if (previous?.status != next.status) {
        // Reset data store when auth status changes
        MarkerDataStoreFactory.resetDataStore();
        // Refresh data
        ref.invalidateSelf();
      }
    });

    return _loadLocations();
  }

  Future<List<Marker>> _loadLocations() async {
    final dataStore = MarkerDataStoreFactory.getDataStore();
    final markers = await dataStore.getAll();

    if (kDebugMode) {
      print("LocationNotifier: Loaded ${markers.length} markers");
    }

    return markers;
  }

  Future<void> addLocation(Marker location) async {
    if (kDebugMode) {
      print("LocationNotifier: Adding location ${location.uuid}");
    }

    try {
      final dataStore = MarkerDataStoreFactory.getDataStore();
      await dataStore.insert(location);

      // Force reload to get the potentially updated UUID from server
      state = const AsyncLoading();
      state = AsyncData(await _loadLocations());

      if (kDebugMode) {
        print("LocationNotifier: Location added, state updated with ${state.value?.length} markers");
      }
    } catch (e) {
      if (kDebugMode) {
        print("LocationNotifier: Error adding location: $e");
      }
      state = AsyncError(e, StackTrace.current);
    }
  }

  Future<void> updateLocation(Marker location) async {
    if (kDebugMode) {
      print("LocationNotifier: Updating location ${location.uuid}");
    }

    try {
      final dataStore = MarkerDataStoreFactory.getDataStore();
      await dataStore.update(location);

      // Reload to ensure we have the latest state
      state = const AsyncLoading();
      state = AsyncData(await _loadLocations());

      if (kDebugMode) {
        print("LocationNotifier: Location updated, state updated with ${state.value?.length} markers");
      }
    } catch (e) {
      if (kDebugMode) {
        print("LocationNotifier: Error updating location: $e");
      }
      state = AsyncError(e, StackTrace.current);
    }
  }

  Future<void> deleteLocation(String id) async {
    if (kDebugMode) {
      print("LocationNotifier: Deleting location $id");
    }

    try {
      final dataStore = MarkerDataStoreFactory.getDataStore();
      await dataStore.delete(id);

      // Reload to ensure we have the latest state
      state = const AsyncLoading();
      state = AsyncData(await _loadLocations());

      if (kDebugMode) {
        print("LocationNotifier: Location deleted, state updated with ${state.value?.length} markers");
      }
    } catch (e) {
      if (kDebugMode) {
        print("LocationNotifier: Error deleting location: $e");
      }
      state = AsyncError(e, StackTrace.current);
    }
  }

  // Method to force sync with server when online
  Future<void> syncWithServer() async {
    if (kDebugMode) {
      print("LocationNotifier: Syncing with server");
    }

    try {
      // Reload data to get latest state
      state = const AsyncLoading();
      state = AsyncData(await _loadLocations());

      if (kDebugMode) {
        print("LocationNotifier: Sync complete, state updated with ${state.value?.length} markers");
      }
    } catch (e) {
      if (kDebugMode) {
        print("LocationNotifier: Error syncing with server: $e");
      }
      state = AsyncError(e, StackTrace.current);
    }
  }
}