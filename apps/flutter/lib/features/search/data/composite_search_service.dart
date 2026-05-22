import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'backends/stedsnavn_search_backend.dart';
import 'location_service.dart';
import 'marker_search_service.dart';
import 'trail_search_service.dart';

/// Forward-search backend (Kartverket Stedsnavn `/navn`). Distinct from
/// `stedsnavnBackendProvider` in `reverse_geocoder.dart`, which is the
/// reverse-geocode `/punkt` backend — same vendor, different endpoint.
final stedsnavnSearchBackendProvider = Provider<StedsnavnSearchBackend>(
  (ref) => StedsnavnSearchBackend(),
);

final compositeSearchServiceProvider = Provider<CompositeSearchService>((ref) {
  final stedsnavn = ref.watch(stedsnavnSearchBackendProvider);
  final markerService = ref.watch(markerSearchServiceProvider);
  final pathService = ref.watch(pathSearchServiceProvider);
  final trailService = ref.watch(trailSearchServiceProvider);
  return CompositeSearchService(
    stedsnavn,
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
