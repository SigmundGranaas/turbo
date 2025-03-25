import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:idb_shim/idb_client_memory.dart';
import 'package:map_app/data/datastore/indexeddb/indexdb.dart';
import 'package:map_app/data/model/marker.dart';
import 'package:map_app/data/search/location_service.dart';
import 'package:map_app/data/search/marker_search_service.dart';

void main() {
  late ShimDBMarkerDataStore dataStore;
  late MarkerSearchService search;

  late IdbFactory idbFactory;

  setUp(() async {
    // Create an in-memory IDB factory
    idbFactory = newIdbFactoryMemory();

    // Inject the factory into our data store
    dataStore = ShimDBMarkerDataStore(idbFactory: idbFactory);

    await dataStore.init();

    final marker = Marker(
      position: const LatLng(40.7128, -74.0060),
      title: 'New York',
      description: 'The Big Apple',
    );

    await dataStore.insert(marker);

    search = MarkerSearchService(dataStore);
  });

  tearDown(() async {
    // Clean up after each test
    await idbFactory.deleteDatabase(ShimDBMarkerDataStore.dbName);
  });

  test('Search by name yields a single result', () async {
   const searchQuery = 'New';
   List<LocationSearchResult> results = await search.findLocationsBy(searchQuery);
   assert(results.length == 1);
  });

  test('Search by invalid name yields no results', () async {
    const searchQuery = 'Old';
    List<LocationSearchResult> results = await search.findLocationsBy(searchQuery);
    assert(results.isEmpty);
  });
}

