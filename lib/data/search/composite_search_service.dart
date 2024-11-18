import 'package:map_app/data/search/location_service.dart';

class CompositeSearchService extends LocationService {

 final LocationService kartverketSearchService;
 final LocationService markerSearchService;

  CompositeSearchService(this.kartverketSearchService, this.markerSearchService);

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    Iterable<LocationService> services = [markerSearchService, kartverketSearchService];
    var futures = <Future<List<LocationSearchResult>>>[];

    for (var s in services) {
      futures.add(s.findLocationsBy(name));
    }

    List<List<LocationSearchResult>> results = await Future.wait(futures);

    return results.expand((element) => element).toList();
  }
}