import 'package:turbo/features/search/data/location_service.dart';

/// Configurable fake [LocationService] for search tests.
///
/// Default behavior: returns whatever was passed to [results] for any query.
/// Set [throwOnQuery] to simulate upstream failures, or pass a [responder]
/// callback to vary results per query (useful for asserting the search term
/// is propagated correctly).
class FakeLocationService implements LocationService {
  List<LocationSearchResult> results;
  Object? throwOnQuery;
  Future<List<LocationSearchResult>> Function(String query)? responder;

  /// Records every query passed in — lets tests assert call order, dedup, etc.
  final List<String> queries = [];

  FakeLocationService({
    this.results = const [],
    this.throwOnQuery,
    this.responder,
  });

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    queries.add(name);
    if (throwOnQuery != null) {
      throw throwOnQuery!;
    }
    if (responder != null) {
      return responder!(name);
    }
    return List.of(results);
  }
}
