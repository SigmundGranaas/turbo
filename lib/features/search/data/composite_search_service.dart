import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'kartverket_location_service.dart';
import 'location_service.dart';
import 'marker_search_service.dart';
import 'trail_search_service.dart';

final kartverketLocationServiceProvider = Provider<KartverketLocationService>((ref) {
  return KartverketLocationService();
});

final compositeSearchServiceProvider = Provider<CompositeSearchService>((ref) {
  final kartverketService = ref.watch(kartverketLocationServiceProvider);
  final markerService = ref.watch(markerSearchServiceProvider);
  final pathService = ref.watch(pathSearchServiceProvider);
  final trailService = ref.watch(trailSearchServiceProvider);
  return CompositeSearchService(
    kartverketService,
    markerService,
    pathService,
    trailService,
  );
});

class CompositeSearchService extends LocationService {
  final LocationService kartverketSearchService;
  final LocationService markerSearchService;
  final LocationService pathSearchService;
  final LocationService trailSearchService;

  CompositeSearchService(
    this.kartverketSearchService,
    this.markerSearchService,
    this.pathSearchService,
    this.trailSearchService,
  );

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    final services = <LocationService>[
      markerSearchService,
      pathSearchService,
      kartverketSearchService,
      trailSearchService,
    ];
    final futures = <Future<List<LocationSearchResult>>>[];
    for (final s in services) {
      futures.add(s.findLocationsBy(name));
    }
    final results = await Future.wait(futures);
    return results.expand((element) => element).toList();
  }
}
