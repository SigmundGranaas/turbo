import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/data/datastore/sqlite/sqlite_marker_datastore.dart';
import 'package:turbo/data/model/marker.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

void main() {
  late SQLiteMarkerDataStore dataStore;
  late Database db;

  setUpAll(() {
    // Initialize FFI
    sqfliteFfiInit();
  });

  setUp(() async {
    // Use an in-memory database for testing
    db = await databaseFactoryFfi.openDatabase(inMemoryDatabasePath);

    // Create a new instance of SQLiteMarkerDataStore for each test
    dataStore = SQLiteMarkerDataStore();

    // Inject the test database
    dataStore.injectDatabase(db);

   await dataStore.createTable(db);
  });

  tearDown(() async {
    // Close the database after each test
    await db.close();
  });

  test('table exists after setup', () async {
    final result = await db.query('sqlite_master', where: 'type = ? AND name = ?', whereArgs: ['table', 'markers']);
    expect(result.length, 1);
    expect(result.first['name'], 'markers');
  });

  test('init creates the markers table', () async {
    final result = await db.query('sqlite_master', where: 'type = ? AND name = ?', whereArgs: ['table', 'markers']);
    expect(result.length, 1);
    expect(result.first['name'], 'markers');
  });

  test('insert and getByUuid', () async {
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