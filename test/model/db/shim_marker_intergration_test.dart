import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:idb_shim/idb_client_memory.dart';
import 'package:turbo/data/datastore/indexeddb/indexdb.dart';
import 'package:turbo/data/model/marker.dart';

void main() {
  late ShimDBMarkerDataStore dataStore;
  late IdbFactory idbFactory;

  setUp(() {
    // Create an in-memory IDB factory
    idbFactory = newIdbFactoryMemory();

    // Inject the factory into our data store
    dataStore = ShimDBMarkerDataStore(idbFactory: idbFactory);
  });

  tearDown(() async {
    // Clean up after each test
    await idbFactory.deleteDatabase(ShimDBMarkerDataStore.dbName);
  });

  test('init creates the database and object store', () async {
    await dataStore.init();

    final db = await idbFactory.open(ShimDBMarkerDataStore.dbName);
    expect(db.objectStoreNames, contains(ShimDBMarkerDataStore.storeName));
    db.close();
  });

  test('insert and getByUuid', () async {
    await dataStore.init();

    final marker = Marker(
      position: const LatLng(40.7128, -74.0060),
      title: 'New York',
      description: 'The Big Apple',
    );

    await dataStore.insert(marker);

    final retrievedMarker = await dataStore.getByUuid(marker.uuid);
    expect(retrievedMarker, isNotNull);
    expect(retrievedMarker!.uuid, equals(marker.uuid));
    expect(retrievedMarker.title, equals(marker.title));
    expect(retrievedMarker.position.latitude, equals(marker.position.latitude));
    expect(retrievedMarker.position.longitude, equals(marker.position.longitude));
  });

  test('getAll returns all inserted markers', () async {
    await dataStore.init();

    final markers = [
      Marker(position: const LatLng(40.7128, -74.0060), title: 'New York'),
      Marker(position: const LatLng(34.0522, -118.2437), title: 'Los Angeles'),
      Marker(position: const LatLng(41.8781, -87.6298), title: 'Chicago'),
    ];

    for (var marker in markers) {
      await dataStore.insert(marker);
    }

    final retrievedMarkers = await dataStore.getAll();
    expect(retrievedMarkers.length, equals(markers.length));

    for (var marker in markers) {
      expect(retrievedMarkers.any((m) => m.uuid == marker.uuid), isTrue);
    }
  });

  test('update modifies an existing marker', () async {
    await dataStore.init();

    final marker = Marker(
      position: const LatLng(51.5074, -0.1278),
      title: 'London',
      description: 'Capital of England',
    );

    await dataStore.insert(marker);

    final updatedMarker = Marker(
      uuid: marker.uuid,
      position: marker.position,
      title: 'London - Updated',
      description: 'The Big Smoke',
    );

    await dataStore.update(updatedMarker);

    final retrievedMarker = await dataStore.getByUuid(marker.uuid);
    expect(retrievedMarker, isNotNull);
    expect(retrievedMarker!.title, equals('London - Updated'));
    expect(retrievedMarker.description, equals('The Big Smoke'));
  });

  test('delete removes a marker', () async {
    await dataStore.init();

    final marker = Marker(
      position: const LatLng(48.8566, 2.3522),
      title: 'Paris',
    );

    await dataStore.insert(marker);

    await dataStore.delete(marker.uuid);

    final retrievedMarker = await dataStore.getByUuid(marker.uuid);
    expect(retrievedMarker, isNull);
  });
}