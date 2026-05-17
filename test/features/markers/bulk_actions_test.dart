import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:turbo/core/api/api_client.dart';
import 'package:turbo/core/connectivity/connectivity_provider.dart';
import 'package:turbo/core/data/database_provider.dart';
import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/markers/data/api_location_service.dart';

import '../../helpers/in_memory_db.dart';
import '../../helpers/wait_for.dart';

/// In-memory fake of the server API — same shape as the one in
/// marker_behavior_test.dart but kept local so each test file is
/// self-contained (and so a hostile change there doesn't break this).
class _FakeApi extends ApiLocationService {
  final List<Marker> serverMarkers = [];
  final List<String> deletedUuids = [];

  _FakeApi() : super(ApiClient(baseUrl: 'http://test'));

  @override
  Future<Marker?> createLocation(Marker marker) async {
    final saved = marker.copyWith(synced: true);
    serverMarkers.add(saved);
    return saved;
  }

  @override
  Future<bool> deleteLocation(String uuid) async {
    serverMarkers.removeWhere((m) => m.uuid == uuid);
    deletedUuids.add(uuid);
    return true;
  }

  @override
  Future<List<Marker>> getLocationsInExtent(LatLng sw, LatLng ne) async =>
      List.of(serverMarkers);

  @override
  Future<List<Marker>> getAllUserLocations() async => List.of(serverMarkers);

  @override
  Future<Marker?> getLocationById(String uuid) async {
    try {
      return serverMarkers.firstWhere((m) => m.uuid == uuid);
    } on StateError {
      return null;
    }
  }
}

class _AuthStub extends AuthStateNotifier {
  @override
  AuthState build() => AuthState(status: AuthStatus.unauthenticated);
}

class _Connectivity extends ConnectivityNotifier {
  @override
  bool build() => true;
}

Marker _m(String title) => Marker(
      title: title,
      position: const LatLng(59.9, 10.7),
    );

void main() {
  late Database db;
  late ProviderContainer container;

  setUp(() async {
    db = await createMarkersDb();
    container = ProviderContainer(overrides: [
      databaseProvider.overrideWith((ref) async => db),
      authStateProvider.overrideWith(() => _AuthStub()),
      connectivityProvider.overrideWith(() => _Connectivity()),
      apiLocationServiceProvider.overrideWithValue(_FakeApi()),
    ]);
    // Keep the autoDispose repository alive.
    container.listen(locationRepositoryProvider, (_, _) {});
    await waitForAsyncData(container, locationRepositoryProvider);
  });

  tearDown(() async {
    container.dispose();
    await db.close();
  });

  group('LocationRepository.deleteMarkers', () {
    test('removes every requested uuid from local storage', () async {
      final repo = container.read(locationRepositoryProvider.notifier);
      await repo.addMarker(_m('Keep'));
      await repo.addMarker(_m('Drop1'));
      await repo.addMarker(_m('Drop2'));

      final before = await waitForAsyncData(
          container, locationRepositoryProvider);
      expect(before, hasLength(3));

      final dropIds = before
          .where((m) => m.title.startsWith('Drop'))
          .map((m) => m.uuid!)
          .toList();
      await repo.deleteMarkers(dropIds);

      final after = await waitForAsyncData(
          container, locationRepositoryProvider);
      expect(after, hasLength(1));
      expect(after.single.title, 'Keep');
    });

    test('empty list is a no-op (state is not touched)', () async {
      final repo = container.read(locationRepositoryProvider.notifier);
      await repo.addMarker(_m('Only'));
      final before = await waitForAsyncData(
          container, locationRepositoryProvider);

      await repo.deleteMarkers(const []);

      final after = container.read(locationRepositoryProvider).value!;
      expect(after.length, before.length);
    });
  });

  group('markerSelectionProvider integration', () {
    test('deleting selected markers + clearing selection leaves the '
        'expected residue', () async {
      final repo = container.read(locationRepositoryProvider.notifier);
      await repo.addMarker(_m('A'));
      await repo.addMarker(_m('B'));
      await repo.addMarker(_m('C'));
      final markers = await waitForAsyncData(
          container, locationRepositoryProvider);

      // Simulate user multi-selecting A + C.
      final selection = container.read(markerSelectionProvider.notifier);
      final byTitle = {for (final m in markers) m.title: m.uuid!};
      selection.toggle(byTitle['A']!);
      selection.toggle(byTitle['C']!);
      expect(container.read(markerSelectionProvider), hasLength(2));

      // Bulk-delete + clear (mirrors what MarkerSelectionBar does).
      await repo.deleteMarkers(
          container.read(markerSelectionProvider).toList());
      selection.clear();

      final after = await waitForAsyncData(
          container, locationRepositoryProvider);
      expect(after.map((m) => m.title), ['B']);
      expect(container.read(markerSelectionProvider), isEmpty);
    });
  });

  group('markersToGeoJson bulk serializer', () {
    test('emits one Feature per marker in the FeatureCollection', () {
      final json = markersToGeoJson([_m('A'), _m('B'), _m('C')]);
      // Count "type": "Feature" entries — the "FeatureCollection" header
      // uses a different value so it won't match.
      expect('"type": "Feature"'.allMatches(json).length, 3);
      expect(json.contains('"type": "FeatureCollection"'), isTrue);
    });

    test('empty input still produces a valid FeatureCollection', () {
      final json = markersToGeoJson(const []);
      expect(json.contains('"features": []'), isTrue);
    });
  });
}
