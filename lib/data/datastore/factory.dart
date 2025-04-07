import 'package:flutter/foundation.dart';
import 'package:map_app/data/datastore/sqlite/sqlite_marker_datastore.dart';
import 'package:map_app/data/datastore/synced_marker_data_store.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'dart:io';


import 'api_location_service.dart';
import 'indexeddb/indexdb.dart';
import 'marker_data_store.dart';

class MarkerDataStoreFactory {
  static MarkerDataStore? _db;
  static const String baseUrl = 'http://localhost:5000';

  static Future<MarkerDataStore> init() async {
    // Create the appropriate local data store based on platform
    final MarkerDataStore localStore;
    if (kIsWeb) {
      localStore = ShimDBMarkerDataStore();
    } else {
      localStore = SQLiteMarkerDataStore();
    }

    // Initialize the local store
    await localStore.init();

    // Check if we're authenticated
    final authState = await _checkAuthState();

    if (authState.$1 && authState.$2 != null) {
      Future<String?> tokenProvider() async {
        final prefs = await SharedPreferences.getInstance();
        return prefs.getString('accessToken');
      }
      // If authenticated, create a synced data store
      final apiService = ApiLocationService(
        baseUrl: baseUrl,
        tokenProvider: tokenProvider,
      );



      final syncedStore = ServerMarkerDataStore(
        localStore: localStore,
        apiService: apiService,
        isOnline: await _checkConnectivity(),
      );

      // Initialize the synced store
      await syncedStore.init();
      _db = syncedStore;
    } else {
      // If not authenticated, just use the local store
      _db = localStore;
    }

    return _db!;
  }

  static MarkerDataStore getDataStore() {
    if (_db == null) {
      throw Exception("MarkerDataStore has not been initialized. Call init() first.");
    }
    return _db!;
  }

  // Reset the data store (e.g., when login/logout occurs)
  static Future<MarkerDataStore> resetDataStore() async {
    _db = null;
    return await init();
  }

  static Future<(bool, String?)> _checkAuthState() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      final isLoggedIn = prefs.getBool('isLoggedIn') ?? false;
      final accessToken = prefs.getString('accessToken');

      return (isLoggedIn, accessToken);
    } catch (e) {
      if (kDebugMode) {
        print("Error checking auth state: $e");
      }
      return (false, null);
    }
  }

  static Future<bool> _checkConnectivity() async {
    if (kIsWeb) {
      // For web, we assume connectivity
      return true;
    }

    try {
      final result = await InternetAddress.lookup('google.com');
      return result.isNotEmpty && result[0].rawAddress.isNotEmpty;
    } on SocketException catch (_) {
      return false;
    }
  }
}