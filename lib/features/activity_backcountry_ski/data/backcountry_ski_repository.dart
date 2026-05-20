import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:logging/logging.dart';

import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/auth/api.dart';

import '../models/backcountry_ski_activity.dart';
import '../models/backcountry_ski_conditions_report.dart';
import '../models/backcountry_ski_details.dart';
import 'backcountry_ski_api.dart';

final backcountrySkiApiProvider = Provider<BackcountrySkiApi>((ref) {
  final apiClient = ref.watch(authenticatedApiClientProvider);
  return BackcountrySkiApi(apiClient);
});

final backcountrySkiActivityProvider =
    FutureProvider.family<BackcountrySkiActivity, String>((ref, id) async {
  final api = ref.watch(backcountrySkiApiProvider);
  return api.getById(id);
});

/// Conditions provider — re-fetched on demand (refresh by invalidating).
final backcountrySkiConditionsProvider =
    FutureProvider.family<BackcountrySkiConditionsReport, String>((ref, id) async {
  return ref.watch(backcountrySkiApiProvider).getConditions(id);
});

final backcountrySkiRepositoryProvider = Provider<BackcountrySkiRepository>((ref) {
  return BackcountrySkiRepository(ref);
});

class BackcountrySkiRepository {
  final Ref _ref;
  final _log = Logger('BackcountrySkiRepository');

  BackcountrySkiRepository(this._ref);

  BackcountrySkiApi get _api => _ref.read(backcountrySkiApiProvider);

  Future<String> create({
    required String name,
    String? description,
    required List<LatLng> route,
    required BackcountrySkiDetails details,
  }) async {
    final id = await _api.create(
      name: name, description: description, route: route, details: details);
    _log.info('Created backcountry ski activity $id');

    // Optimistic cross-kind summary upsert so the route renders on the
    // map immediately. The server-authoritative version + timestamps
    // arrive on the next delta-sync round.
    final wkt = _routeWkt(route);
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).upsertLocal(
          activities.ActivitySummary(
            id: id,
            kind: 'backcountry_ski',
            name: name,
            geometry: activities.ActivityGeometry.fromServer(
              wkt: wkt,
              geometryKind: 'LINESTRING',
            ),
            iconKey: 'backcountry_ski',
            colorHex: '#7A3CCB',
            updatedAt: DateTime.now().toUtc(),
            version: 1,
          ),
        );
    return id;
  }

  Future<void> update({
    required String id,
    String? name,
    String? description,
    List<LatLng>? route,
    BackcountrySkiDetails? details,
  }) async {
    await _api.update(id: id, name: name, description: description, route: route, details: details);
    _ref.invalidate(backcountrySkiActivityProvider(id));
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).refresh();
  }

  Future<void> delete(String id) async {
    await _api.delete(id);
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).removeLocal(id);
  }

  static String _routeWkt(List<LatLng> route) {
    if (route.isEmpty) return 'LINESTRING EMPTY';
    return 'LINESTRING(${route.map((p) => '${p.longitude} ${p.latitude}').join(', ')})';
  }
}
