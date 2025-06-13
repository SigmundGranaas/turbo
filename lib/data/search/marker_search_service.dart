import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/datastore/marker_data_store.dart';
import 'package:turbo/data/search/location_service.dart';
import 'package:turbo/data/state/providers/location_repository.dart';

import '../model/marker.dart';

final markerSearchServiceProvider = Provider<MarkerSearchService>((ref) {
  return MarkerSearchService(ref);
});

class MarkerSearchService extends LocationService {
  final Ref _ref;

  MarkerSearchService(this._ref);

  MarkerDataStore get _store => _ref.read(localMarkerDataStoreProvider);

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    // findByName is not part of MarkerDataStore, so we fetch all and filter.
    // This assumes the local store is already initialized by LocationRepository.
    final allMarkers = await _store.getAll();
    final searchTerm = name.toLowerCase();
    final List<Marker> res = allMarkers
        .where((marker) => marker.title.toLowerCase().contains(searchTerm))
        .toList();

    final List<LocationSearchResult> mapped = res.map((el) => from(el)).toList();
    return mapped;
  }

  LocationSearchResult from(Marker marker){
    return LocationSearchResult(title: marker.title, description: marker.description, position: marker.position, icon: marker.icon);
  }
}