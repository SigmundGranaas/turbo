import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/search/location_service.dart';
import 'package:turbo/data/state/providers/location_repository.dart';

import '../model/marker.dart';

final markerSearchServiceProvider = Provider<MarkerSearchService>((ref) {
  return MarkerSearchService(ref);
});

class MarkerSearchService extends LocationService {
  final Ref _ref;

  MarkerSearchService(this._ref);

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    try {
      // Safely await the fully initialized data store.
      final store = await _ref.read(localMarkerDataStoreProvider.future);

      final allMarkers = await store.getAll();
      final searchTerm = name.toLowerCase();
      final List<Marker> res = allMarkers
          .where((marker) => marker.title.toLowerCase().contains(searchTerm))
          .toList();

      final List<LocationSearchResult> mapped = res.map((el) => from(el)).toList();
      return mapped;
    } catch (e) {
      // If the database fails, we don't want to crash the search.
      // Just return no local results.
      if (kDebugMode) {
        print("Error searching local markers: $e");
      }
      return [];
    }
  }

  LocationSearchResult from(Marker marker){
    // Added source for better debugging and potential UI differentiation
    return LocationSearchResult(
        title: marker.title,
        description: marker.description,
        position: marker.position,
        icon: marker.icon,
        source: 'local'
    );
  }
}