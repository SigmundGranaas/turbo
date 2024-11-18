import 'package:map_app/data/datastore/marker_data_store.dart';
import 'package:map_app/data/search/location_service.dart';

import '../model/marker.dart';

class MarkerSearchService extends LocationService {
  MarkerDataStore? store;
  
  MarkerSearchService(this.store);

  void injectDatabase(MarkerDataStore database) {
    store = database;
  }

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    if(store == null){
      throw Exception("Cannot search with no initialized db.");
    }

    final res = await store!.findByName(name);
    final List<LocationSearchResult> mapped = res.map((el) => from(el)).toList();
    return mapped;
  }

  LocationSearchResult from(Marker marker){
    return LocationSearchResult(title: marker.title, description: marker.description, position: marker.position, icon: marker.icon);
  }
}