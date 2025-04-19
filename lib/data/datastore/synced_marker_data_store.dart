import 'package:flutter/foundation.dart';
import '../model/marker.dart';
import 'marker_data_store.dart';
import 'api_location_service.dart';

/// A streamlined marker store implementation that handles synchronization between
/// local storage and server without unnecessary complexity
class ServerMarkerDataStore implements MarkerDataStore {
  final MarkerDataStore _localStore;
  final ApiLocationService _apiService;
  final bool _isOnline;

  // Flag to prevent concurrent syncs
  bool _isSyncing = false;

  ServerMarkerDataStore({
    required MarkerDataStore localStore,
    required ApiLocationService apiService,
    bool isOnline = true,
  }) :
        _localStore = localStore,
        _apiService = apiService,
        _isOnline = isOnline;

  @override
  Future<void> init() async {
    try {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Initializing");
      }

      // Initialize local storage
      await _localStore.init();

      // Sync with server if online
      if (_isOnline) {
        await sync();
      }

      if (kDebugMode) {
        print("ServerMarkerDataStore: Initialized");
      }
    } catch (e) {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Error during initialization: $e");
      }
    }
  }

  @override
  Future<void> insert(Marker marker) async {
    try {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Creating marker ${marker.uuid}");
      }

      // Store locally first for immediate feedback
      await _localStore.insert(marker);

      // If online, send to server
      if (_isOnline) {
        try {
          final serverMarker = await _apiService.createLocation(marker);

          if (serverMarker != null) {
            if (kDebugMode) {
              print("ServerMarkerDataStore: Created on server with ID: ${serverMarker.uuid}");
            }

            // Replace local marker with server version if needed
            if (serverMarker.uuid != marker.uuid) {
              await _localStore.delete(marker.uuid);
              await _localStore.insert(serverMarker);
            } else {
              // Just update the synced status
              await _localStore.update(serverMarker);
            }
          }
        } catch (e) {
          if (kDebugMode) {
            print("ServerMarkerDataStore: Error creating marker on server: $e");
          }
        }
      }
    } catch (e) {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Error inserting marker: $e");
      }
      rethrow;
    }
  }

  @override
  Future<Marker?> getByUuid(String uuid) async {
    try {
      return await _localStore.getByUuid(uuid);
    } catch (e) {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Error getting marker by UUID: $e");
      }
      rethrow;
    }
  }

  @override
  Future<List<Marker>> findByName(String name) async {
    try {
      final markers = await _localStore.getAll();
      final searchTerm = name.toLowerCase();
      return markers.where((marker) =>
          marker.title.toLowerCase().contains(searchTerm)
      ).toList();
    } catch (e) {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Error finding markers by name: $e");
      }
      return [];
    }
  }

  @override
  Future<List<Marker>> getAll() async {
    try {
      return await _localStore.getAll();
    } catch (e) {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Error getting all markers: $e");
      }
      return [];
    }
  }

  @override
  Future<void> update(Marker marker) async {
    try {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Updating marker: ${marker.uuid}");
      }

      // Update locally first
      await _localStore.update(marker);

      // If online, update on server
      if (_isOnline) {
        try {
          final success = await _apiService.updateLocationPosition(marker);

          if (kDebugMode) {
            if (success) {
              print("ServerMarkerDataStore: Marker updated on server");
            } else {
              print("ServerMarkerDataStore: Failed to update marker on server");
            }
          }
        } catch (e) {
          if (kDebugMode) {
            print("ServerMarkerDataStore: Error updating marker on server: $e");
          }
        }
      }
    } catch (e) {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Error updating marker: $e");
      }
      rethrow;
    }
  }

  @override
  Future<void> delete(String uuid) async {
    try {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Deleting marker: $uuid");
      }

      // Delete locally first
      await _localStore.delete(uuid);

      // If online, delete from server
      if (_isOnline) {
        try {
          final success = await _apiService.deleteLocation(uuid);

          if (kDebugMode) {
            if (success) {
              print("ServerMarkerDataStore: Marker deleted on server");
            } else {
              print("ServerMarkerDataStore: Failed to delete marker from server");
            }
          }
        } catch (e) {
          if (kDebugMode) {
            print("ServerMarkerDataStore: Error deleting marker from server: $e");
          }
        }
      }
    } catch (e) {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Error deleting marker: $e");
      }
      rethrow;
    }
  }

  // Public method to sync with server
  Future<void> sync() async {
    if (!_isOnline || _isSyncing) return;

    _isSyncing = true;

    try {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Syncing with server");
      }

      // Get markers from server
      final serverMarkers = await _apiService.getLocationsInExtent(-180, -90, 180, 90);
      if (kDebugMode) {
        print("ServerMarkerDataStore: Retrieved ${serverMarkers.length} markers from server");
      }

      // Get local markers
      final localMarkers = await _localStore.getAll();
      if (kDebugMode) {
        print("ServerMarkerDataStore: Found ${localMarkers.length} local markers");
      }

      // Create lookup maps
      final Map<String, Marker> serverMap = {
        for (var marker in serverMarkers) marker.uuid: marker
      };
      final Map<String, Marker> localMap = {
        for (var marker in localMarkers) marker.uuid: marker
      };

      // 1. Process server markers - add or update locally
      for (final serverMarker in serverMarkers) {
        if (!localMap.containsKey(serverMarker.uuid)) {
          // New server marker - add locally
          await _localStore.insert(serverMarker);
        } else {
          // Existing marker - update if needed
          final localMarker = localMap[serverMarker.uuid]!;
          if (_needsUpdate(localMarker, serverMarker)) {
            await _localStore.update(serverMarker);
          }
        }
      }

      // 2. Process local markers - create on server if needed
      for (final localMarker in localMarkers) {
        if (!serverMap.containsKey(localMarker.uuid) && !_hasMatchingSignature(localMarker, serverMarkers)) {
          try {
            // Create on server
            final serverMarker = await _apiService.createLocation(localMarker);
            if (serverMarker != null) {
              // Replace local marker with server version if UUID changed
              if (serverMarker.uuid != localMarker.uuid) {
                await _localStore.delete(localMarker.uuid);
                await _localStore.insert(serverMarker);
              } else {
                await _localStore.update(serverMarker);
              }
            }
          } catch (e) {
            if (kDebugMode) {
              print("ServerMarkerDataStore: Error creating local marker on server: $e");
            }
          }
        }
      }
    } catch (e) {
      if (kDebugMode) {
        print("ServerMarkerDataStore: Error syncing with server: $e");
      }
    } finally {
      _isSyncing = false;
    }
  }

  // Check if a marker needs updating
  bool _needsUpdate(Marker local, Marker server) {
    return local.title != server.title ||
        local.description != server.description ||
        local.icon != server.icon ||
        local.position.latitude != server.position.latitude ||
        local.position.longitude != server.position.longitude ||
        local.synced != server.synced;
  }

  // Check if a local marker already exists on server with different ID
  bool _hasMatchingSignature(Marker localMarker, List<Marker> serverMarkers) {
    final localSignature = _getMarkerSignature(localMarker);

    for (final serverMarker in serverMarkers) {
      if (_getMarkerSignature(serverMarker) == localSignature) {
        return true;
      }
    }

    return false;
  }

  // Get a marker signature for comparison
  String _getMarkerSignature(Marker marker) {
    return "${marker.title}|${marker.position.latitude}|${marker.position.longitude}|${marker.icon}";
  }
}