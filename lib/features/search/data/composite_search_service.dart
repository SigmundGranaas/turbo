import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/saved_paths/data/path_search_service.dart';
import 'kartverket_location_service.dart';
import 'location_service.dart';
import 'marker_search_service.dart';

final kartverketLocationServiceProvider = Provider<KartverketLocationService>((ref) {
  return KartverketLocationService();
});

final compositeSearchServiceProvider = Provider<CompositeSearchService>((ref) {
  final kartverketService = ref.watch(kartverketLocationServiceProvider);
  final markerService = ref.watch(markerSearchServiceProvider);
  final pathService = ref.watch(pathSearchServiceProvider);
  return CompositeSearchService(kartverketService, markerService, pathService);
});

class CompositeSearchService extends LocationService {
  final LocationService kartverketSearchService;
  final LocationService markerSearchService;
  final LocationService pathSearchService;

  CompositeSearchService(this.kartverketSearchService, this.markerSearchService, this.pathSearchService);

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    Iterable<LocationService> services = [markerSearchService, pathSearchService, kartverketSearchService];
    var futures = <Future<List<LocationSearchResult>>>[];

    for (var s in services) {
      futures.add(s.findLocationsBy(name));
    }

    List<List<LocationSearchResult>> results = await Future.wait(futures);

    return results.expand((element) => element).toList();
  }
}