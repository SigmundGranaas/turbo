import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/search/data/location_service.dart';
import 'saved_path_repository.dart';

final pathSearchServiceProvider = Provider<PathSearchService>((ref) {
  return PathSearchService(ref);
});

class PathSearchService extends LocationService {
  final Ref _ref;

  PathSearchService(this._ref);

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    try {
      final store = await _ref.read(localSavedPathDataStoreProvider.future);
      final allPaths = await store.getAll();
      final searchTerm = name.toLowerCase();
      final results = allPaths
          .where((path) => path.title.toLowerCase().contains(searchTerm))
          .toList();

      return results.map((path) => LocationSearchResult(
        title: path.title,
        description: path.description,
        position: path.points.first,
        source: 'saved_path',
      )).toList();
    } catch (e) {
      if (kDebugMode) {
        print("Error searching saved paths: $e");
      }
      return [];
    }
  }
}
