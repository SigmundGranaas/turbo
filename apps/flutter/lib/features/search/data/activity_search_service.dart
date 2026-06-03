import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';

import 'package:turbo/features/activities/api.dart';

import 'location_service.dart';

final _log = Logger('ActivitySearchService');

final activitySearchServiceProvider = Provider<ActivitySearchService>((ref) {
  return ActivitySearchService(ref);
});

/// Search backend over the user's activities (hikes, fishing, …). Mirrors
/// [MarkerSearchService] / [PathSearchService]: filters the in-memory activity
/// summaries by name and maps each hit to its geometry's first point so it can
/// be panned to (and, via the selection seam, acted on).
class ActivitySearchService extends LocationService {
  final Ref _ref;

  ActivitySearchService(this._ref);

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) async {
    try {
      final summaries =
          _ref.read(activitySummariesRepositoryProvider).value ?? const {};
      final term = name.toLowerCase();
      final results = <LocationSearchResult>[];
      for (final summary in summaries.values) {
        if (!summary.name.toLowerCase().contains(term)) continue;
        final position = summary.geometry.firstPoint;
        if (position == null) continue;
        results.add(LocationSearchResult(
          title: summary.name,
          position: position,
          source: 'activity',
        ));
      }
      return results;
    } catch (e) {
      _log.warning('Error searching activities', e);
      return [];
    }
  }
}
