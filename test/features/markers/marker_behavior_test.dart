import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/core/api/api_client.dart';
import 'package:turbo/core/connectivity/connectivity_provider.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/markers/data/api_location_service.dart';
import 'package:turbo/features/markers/data/location_repository.dart';
import 'package:turbo/features/markers/data/pending_delete_store.dart';
import 'package:turbo/features/markers/data/sqlite_marker_datastore.dart';
import 'package:turbo/features/markers/data/viewport_marker_provider.dart';
import 'package:turbo/features/markers/models/marker.dart';
import 'package:flutter_map/flutter_map.dart' as fm;

// ---------------------------------------------------------------------------
// Fakes
// ---------------------------------------------------------------------------

/// Controllable fake server. All methods operate on in-memory lists.
class FakeApiLocationService extends ApiLocationService {
  final List<Marker> serverMarkers = [];
  final List<String> deletedUuids = [];

  /// When true, every method throws to simulate network failure.
  bool shouldFail = false;

  FakeApiLocationService() : super(ApiClient(baseUrl: 'http://test'));

  @override
  Future<Marker?> createLocation(Marker marker) async {
    if (shouldFail) throw Exception('Network error');
    // Simulate server assigning the same uuid back (simplified).
    final saved = marker.copyWith(synced: true);
    serverMarkers.add(saved);
    return saved;
  }

  @override
  Future<Marker?> updateLocation(Marker marker) async {
    if (shouldFail) throw Exception('Network error');
    serverMarkers.removeWhere((m) => m.uuid == marker.uuid);
    final updated = marker.copyWith(synced: true);
    serverMarkers.add(updated);
    return updated;
  }

  @override
  Future<bool> deleteLocation(String uuid) async {
    if (shouldFail) throw Exception('Network error');
    serverMarkers.removeWhere((m) => m.uuid == uuid);
    deletedUuids.add(uuid);
    return true;
  }

  @override
  Future<List<Marker>> getLocationsInExtent(
      LatLng southwest, LatLng northeast) async {
    if (shouldFail) throw Exception('Network error');
    return List.of(serverMarkers);
  }

  @override
  Future<List<Marker>> getAllUserLocations() async {
    if (shouldFail) throw Exception('Network error');
    return List.of(serverMarkers);
  }

  @override
  Future<Marker?> getLocationById(String uuid) async {
    if (shouldFail) throw Exception('Network error');
    try {
      return serverMarkers.firstWhere((m) => m.uuid == uuid);
    } on StateError {
      return null;
    }
  }
}

/// Test auth notifier that does not create real ApiClient / AppLinks.
class TestAuthNotifier extends AuthStateNotifier {
  @override
  AuthState build() => AuthState(status: AuthStatus.unauthenticated);

  void setAuthenticated() {
    state = AuthState(status: AuthStatus.authenticated, email: 'test@test.com');
  }

  void setUnauthenticated() {
    state = AuthState(status: AuthStatus.unauthenticated);
  }
}

/// Controllable connectivity notifier — avoids real connectivity_plus.
class TestConnectivityNotifier extends ConnectivityNotifier {
  final bool _initial;
  TestConnectivityNotifier([this._initial = true]);

  @override
  bool build() => _initial;

  void setOnline() => state = true;
  void setOffline() => state = false;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

Marker _makeMarker({
  String? uuid,
  String title = 'Test',
  double lat = 59.9,
  double lng = 10.7,
  bool synced = false,
}) =>
    Marker(
      uuid: uuid,
      title: title,
      position: LatLng(lat, lng),
      synced: synced,
    );

/// Wait for the LocationRepository state to settle to data (not loading).
Future<List<Marker>> _waitForData(ProviderContainer container) async {
  // Poll until state transitions out of loading.
  for (var i = 0; i < 100; i++) {
    await Future.delayed(const Duration(milliseconds: 20));
    final s = container.read(locationRepositoryProvider);
    if (s is AsyncData<List<Marker>>) return s.value;
    if (s is AsyncError) throw (s as AsyncError).error;
  }
  throw TimeoutException('LocationRepository did not settle');
}

Future<Database> _createTestDb() async {
  final db = await databaseFactory.openDatabase(inMemoryDatabasePath);
  await db.execute('''
    CREATE TABLE $markersTable(
      uuid TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      description TEXT,
      icon TEXT,
      latitude REAL NOT NULL,
      longitude REAL NOT NULL,
      synced INTEGER NOT NULL DEFAULT 0
    )
  ''');
  await db.execute(
      'CREATE INDEX idx_markers_coords ON $markersTable(latitude, longitude)');
  await db.execute(
      'CREATE INDEX idx_markers_synced ON $markersTable(synced)');
  await db.execute('''
    CREATE TABLE $pendingDeletesTable(
      uuid TEXT PRIMARY KEY,
      created_at TEXT NOT NULL
    )
  ''');
  return db;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

void main() {
  late Database db;
  late ProviderContainer container;
  late FakeApiLocationService fakeApi;
  late TestAuthNotifier testAuth;
  late TestConnectivityNotifier testConnectivity;

  setUpAll(() {
    sqfliteFfiInit();
    databaseFactory = databaseFactoryFfi;
  });

  setUp(() async {
    db = await _createTestDb();
    fakeApi = FakeApiLocationService();
    testAuth = TestAuthNotifier();
    testConnectivity = TestConnectivityNotifier(true);

    container = ProviderContainer(overrides: [
      databaseProvider.overrideWith((ref) async => db),
      authStateProvider.overrideWith(() => testAuth),
      connectivityProvider.overrideWith(() => testConnectivity),
      apiLocationServiceProvider.overrideWithValue(fakeApi),
    ]);

    // Keep the autoDispose provider alive for the duration of the test.
    container.listen(locationRepositoryProvider, (_, _) {});

    // Let the repository's _initState() complete.
    await _waitForData(container);
  });

  tearDown(() async {
    container.dispose();
    await db.close();
  });

  // -----------------------------------------------------------------------
  // 1. Markers persist locally
  // -----------------------------------------------------------------------
  group('Markers persist locally', () {
    test('create a marker while unauthenticated -> appears in state', () async {
      final marker = _makeMarker(title: 'Home');

      final repo = container.read(locationRepositoryProvider.notifier);
      await repo.addMarker(marker);
      final markers = await _waitForData(container);

      expect(markers, isNotEmpty);
      expect(markers.any((m) => m.title == 'Home'), isTrue);
    });

    test('created marker survives a fresh DB query', () async {
      final marker = _makeMarker(title: 'Office');
      final repo = container.read(locationRepositoryProvider.notifier);
      await repo.addMarker(marker);
      await _waitForData(container);

      // Query directly from the database — bypasses in-memory state.
      final store = SQLiteMarkerDataStore(db);
      final fromDb = await store.getAll();
      expect(fromDb.any((m) => m.title == 'Office'), isTrue);
    });

    test('update a marker -> changes reflected', () async {
      final marker = _makeMarker(title: 'Before');
      final repo = container.read(locationRepositoryProvider.notifier);
      await repo.addMarker(marker);
      await _waitForData(container);

      await repo.updateMarker(marker.copyWith(title: 'After'));
      final markers = await _waitForData(container);

      expect(markers.any((m) => m.title == 'After'), isTrue);
      expect(markers.any((m) => m.title == 'Before'), isFalse);
    });

    test('delete a marker -> gone from state', () async {
      final marker = _makeMarker(title: 'Temp');
      final repo = container.read(locationRepositoryProvider.notifier);
      await repo.addMarker(marker);
      var markers = await _waitForData(container);
      expect(markers.any((m) => m.title == 'Temp'), isTrue);

      await repo.deleteMarker(marker.uuid);
      markers = await _waitForData(container);
      expect(markers.any((m) => m.title == 'Temp'), isFalse);
    });
  });

  // -----------------------------------------------------------------------
  // 2. Deleting a marker offline keeps it deleted
  // -----------------------------------------------------------------------
  group('Deleting a marker offline keeps it deleted', () {
    test('synced marker deleted offline stays deleted after reconnect',
        () async {
      // Start authenticated with a synced marker.
      final marker = _makeMarker(title: 'Remote', synced: true);
      fakeApi.serverMarkers.add(marker);
      final store = SQLiteMarkerDataStore(db);
      await store.insert(marker);

      testAuth.setAuthenticated();
      await _waitForData(container);

      // Go offline, then delete.
      testConnectivity.setOffline();
      fakeApi.shouldFail = true;

      final repo = container.read(locationRepositoryProvider.notifier);
      await repo.deleteMarker(marker.uuid);
      var markers = await _waitForData(container);
      expect(markers.any((m) => m.uuid == marker.uuid), isFalse,
          reason: 'Marker should be gone from state after offline delete');

      // Verify it was queued for pending delete.
      final pendingStore = SqlitePendingDeleteStore(db);
      final pending = await pendingStore.getAll();
      expect(pending, contains(marker.uuid));

      // Come back online — the connectivity listener should drain the queue.
      fakeApi.shouldFail = false;
      fakeApi.serverMarkers.remove(marker); // already deleted locally
      testConnectivity.setOnline();
      // Wait for the reconnect sync to process.
      await _waitForData(container);
      // Give extra time for _processOfflineQueue async chain.
      await Future.delayed(const Duration(milliseconds: 200));

      markers = await _waitForData(container);
      expect(markers.any((m) => m.uuid == marker.uuid), isFalse,
          reason: 'Marker must not reappear after reconnection');

      // Pending deletes should be drained.
      final pendingAfter = await pendingStore.getAll();
      expect(pendingAfter, isEmpty);
    });
  });

  // -----------------------------------------------------------------------
  // 3. Local markers upload to the server on login
  // -----------------------------------------------------------------------
  group('Local markers upload to the server on login', () {
    test('unsynced markers POST to server on authentication', () async {
      // Create markers while unauthenticated.
      final repo = container.read(locationRepositoryProvider.notifier);
      final m1 = _makeMarker(title: 'Spot A');
      final m2 = _makeMarker(title: 'Spot B');
      await repo.addMarker(m1);
      await repo.addMarker(m2);
      await _waitForData(container);

      // Verify they are unsynced locally.
      final store = SQLiteMarkerDataStore(db);
      var unsynced = await store.getUnsynced();
      expect(unsynced.length, 2);

      // Authenticate -> triggers sync.
      testAuth.setAuthenticated();
      await _waitForData(container);
      // Extra time for the sync-then-load chain.
      await Future.delayed(const Duration(milliseconds: 300));
      await _waitForData(container);

      // Server received the markers.
      expect(fakeApi.serverMarkers.any((m) => m.title == 'Spot A'), isTrue);
      expect(fakeApi.serverMarkers.any((m) => m.title == 'Spot B'), isTrue);

      // Local copies now marked as synced.
      unsynced = await store.getUnsynced();
      expect(unsynced, isEmpty);
    });
  });

  // -----------------------------------------------------------------------
  // 4. Server sync preserves unsynced local markers
  // -----------------------------------------------------------------------
  group('Server sync preserves unsynced local markers', () {
    test(
        'server fetch does not wipe unsynced local marker',
        () async {
      // Seed an unsynced local marker.
      final localMarker = _makeMarker(title: 'My Draft', synced: false);
      final store = SQLiteMarkerDataStore(db);
      await store.insert(localMarker);

      // Server has a different marker.
      final serverMarker = _makeMarker(
        uuid: 'server-uuid-1',
        title: 'Server Marker',
        synced: true,
      );
      fakeApi.serverMarkers.add(serverMarker);

      // Authenticate -> _syncLocalDataOnLogin uploads draft, then _loadData fetches server.
      testAuth.setAuthenticated();
      await _waitForData(container);
      await Future.delayed(const Duration(milliseconds: 300));
      final markers = await _waitForData(container);

      // The server marker should be present.
      expect(markers.any((m) => m.title == 'Server Marker'), isTrue);
      // The local draft should also still exist (it was uploaded and comes back from server).
      // After sync the local draft was uploaded, so the server's copy has it too.
      expect(
          markers.any((m) => m.title == 'My Draft'), isTrue,
          reason: 'Unsynced local marker should survive server sync');
    });

    test('server returns empty -> unsynced marker still present', () async {
      final localMarker = _makeMarker(title: 'Lonely Draft', synced: false);
      final store = SQLiteMarkerDataStore(db);
      await store.insert(localMarker);

      // Server is empty.
      fakeApi.serverMarkers.clear();

      testAuth.setAuthenticated();
      await _waitForData(container);
      await Future.delayed(const Duration(milliseconds: 300));
      final markers = await _waitForData(container);

      expect(markers.any((m) => m.title == 'Lonely Draft'), isTrue,
          reason: 'Unsynced marker must survive even when server is empty');
    });
  });

  // -----------------------------------------------------------------------
  // 5. Offline changes sync automatically on reconnect
  // -----------------------------------------------------------------------
  group('Offline changes sync automatically on reconnect', () {
    test('create + delete offline, both sync on reconnect', () async {
      // Start authenticated and online with one synced marker.
      final existing = _makeMarker(
          uuid: 'to-delete', title: 'Existing', synced: true);
      fakeApi.serverMarkers.add(existing);
      final store = SQLiteMarkerDataStore(db);
      await store.insert(existing);

      testAuth.setAuthenticated();
      await _waitForData(container);

      // Go offline.
      testConnectivity.setOffline();
      fakeApi.shouldFail = true;

      final repo = container.read(locationRepositoryProvider.notifier);

      // Create a new marker offline.
      final newMarker = _makeMarker(title: 'Created Offline');
      await repo.addMarker(newMarker);
      await _waitForData(container);

      // Delete the existing marker offline.
      await repo.deleteMarker(existing.uuid);
      await _waitForData(container);

      // Go back online.
      fakeApi.shouldFail = false;
      testConnectivity.setOnline();
      // Wait for _processOfflineQueue.
      await Future.delayed(const Duration(milliseconds: 500));
      final markers = await _waitForData(container);

      // The new marker was uploaded.
      expect(fakeApi.serverMarkers.any((m) => m.title == 'Created Offline'),
          isTrue,
          reason: 'Offline-created marker should upload on reconnect');

      // The deleted marker was sent as a server delete.
      expect(fakeApi.deletedUuids, contains('to-delete'),
          reason: 'Offline delete should be sent to server on reconnect');

      // Final local state: new marker present, deleted marker gone.
      expect(markers.any((m) => m.title == 'Created Offline'), isTrue);
      expect(markers.any((m) => m.uuid == 'to-delete'), isFalse);
    });
  });

  // -----------------------------------------------------------------------
  // 6. Viewport falls back to local when API unreachable
  // -----------------------------------------------------------------------
  group('Viewport falls back to local when API unreachable', () {
    test('offline viewport query returns local markers', () async {
      // Seed local DB directly.
      final store = SQLiteMarkerDataStore(db);
      final m = _makeMarker(title: 'Local Pin', lat: 60.0, lng: 11.0);
      await store.insert(m);

      testAuth.setAuthenticated();
      await _waitForData(container);

      testConnectivity.setOffline();

      // Listen to viewport provider.
      container.listen(viewportMarkerNotifierProvider, (_, _) {});
      final viewportNotifier =
          container.read(viewportMarkerNotifierProvider.notifier);

      // Query a viewport that includes our marker.
      final bounds = fm.LatLngBounds(
        const LatLng(59.0, 10.0),
        const LatLng(61.0, 12.0),
      );
      viewportNotifier.loadMarkersInViewport(bounds, 10);

      // Wait for debounce (500ms) + processing.
      await Future.delayed(const Duration(milliseconds: 800));

      final viewportState = container.read(viewportMarkerNotifierProvider);
      expect(viewportState.value, isNotNull);
      expect(viewportState.value!.any((m) => m.title == 'Local Pin'), isTrue,
          reason: 'Should fall back to local store when offline');
    });

    test('API throws -> viewport falls back to local', () async {
      final store = SQLiteMarkerDataStore(db);
      final m = _makeMarker(title: 'Fallback Pin', lat: 55.0, lng: 8.0);
      await store.insert(m);

      testAuth.setAuthenticated();
      await _waitForData(container);

      // Online but API throws.
      testConnectivity.setOnline();
      fakeApi.shouldFail = true;

      container.listen(viewportMarkerNotifierProvider, (_, _) {});
      final viewportNotifier =
          container.read(viewportMarkerNotifierProvider.notifier);

      // Use different bounds than the previous test to avoid module-level cache hit.
      final bounds = fm.LatLngBounds(
        const LatLng(54.0, 7.0),
        const LatLng(56.0, 9.0),
      );
      viewportNotifier.loadMarkersInViewport(bounds, 12);

      await Future.delayed(const Duration(milliseconds: 800));

      final viewportState = container.read(viewportMarkerNotifierProvider);
      expect(viewportState.value, isNotNull);
      expect(
          viewportState.value!.any((m) => m.title == 'Fallback Pin'), isTrue,
          reason: 'Should fall back to local when API throws');
    });
  });

  // -----------------------------------------------------------------------
  // 7. Manual sync reconciles bidirectionally
  // -----------------------------------------------------------------------
  group('Manual sync reconciles bidirectionally', () {
    test('drains pending deletes, uploads unsynced, adds server-only markers',
        () async {
      testAuth.setAuthenticated();
      await _waitForData(container);

      final store = SQLiteMarkerDataStore(db);
      final pendingStore = SqlitePendingDeleteStore(db);

      // 1. An unsynced local marker.
      final unsyncedLocal =
          _makeMarker(uuid: 'local-only', title: 'Unsynced', synced: false);
      await store.insert(unsyncedLocal);

      // 2. A pending delete.
      await pendingStore.add('pending-del-uuid');

      // 3. A server-only marker that we don't have locally.
      final serverOnly = _makeMarker(
        uuid: 'server-only',
        title: 'From Server',
        synced: true,
      );
      fakeApi.serverMarkers.add(serverOnly);

      // Trigger manual sync.
      final repo = container.read(locationRepositoryProvider.notifier);
      await repo.triggerManualSync();
      final markers = await _waitForData(container);

      // Pending delete was drained.
      final remainingPending = await pendingStore.getAll();
      expect(remainingPending, isEmpty,
          reason: 'Pending deletes should be drained');
      expect(fakeApi.deletedUuids, contains('pending-del-uuid'));

      // Unsynced local was uploaded.
      expect(
          fakeApi.serverMarkers.any((m) => m.title == 'Unsynced'), isTrue,
          reason: 'Unsynced local marker should be uploaded');

      // Server-only marker now present locally.
      expect(markers.any((m) => m.title == 'From Server'), isTrue,
          reason: 'Server-only marker should be added locally');

      // The uploaded local marker also present.
      expect(markers.any((m) => m.title == 'Unsynced'), isTrue);
    });
  });
}
