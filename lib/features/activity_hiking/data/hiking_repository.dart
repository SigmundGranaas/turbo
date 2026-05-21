import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:logging/logging.dart';

import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/auth/api.dart';

import '../models/hiking_activity.dart';
import '../models/hiking_conditions_report.dart';
import '../models/hiking_details.dart';
import 'hiking_api.dart';

final hikingApiProvider = Provider<HikingApi>((ref) {
  final apiClient = ref.watch(authenticatedApiClientProvider);
  return HikingApi(apiClient);
});

final hikingActivityProvider =
    FutureProvider.family<HikingActivity, String>((ref, id) async {
  return activities.fetchActivityCached<HikingActivity>(
    ref: ref,
    kindUrlSlug: 'hiking',
    kindKey: 'hiking',
    activityId: id,
    fromJson: HikingActivity.fromJson,
  );
});

final hikingConditionsProvider =
    FutureProvider.family<HikingConditionsReport, String>((ref, id) async {
  return activities.fetchConditionsCached<HikingConditionsReport>(
    ref: ref,
    kindUrlSlug: 'hiking',
    kindKey: 'hiking',
    activityId: id,
    fromJson: HikingConditionsReport.fromJson,
  );
});

final hikingRepositoryProvider = Provider<HikingRepository>((ref) => HikingRepository(ref));

class HikingRepository {
  final Ref _ref;
  final _log = Logger('HikingRepository');
  HikingRepository(this._ref);

  HikingApi get _api => _ref.read(hikingApiProvider);

  Future<String> create({
    required String name, String? description,
    required List<LatLng> route, required HikingDetails details,
  }) async {
    final id = await _api.create(name: name, description: description, route: route, details: details);
    _log.info('Created hiking activity $id');
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).upsertLocal(
          activities.ActivitySummary(
            id: id, kind: 'hiking', name: name,
            geometry: activities.ActivityGeometry.fromServer(wkt: _wkt(route), geometryKind: 'LINESTRING'),
            iconKey: 'hiking', colorHex: '#2E7D32',
            updatedAt: DateTime.now().toUtc(), version: 1,
          ),
        );
    return id;
  }

  Future<void> update({
    required String id, String? name, String? description,
    List<LatLng>? route, HikingDetails? details,
  }) async {
    await _api.update(id: id, name: name, description: description, route: route, details: details);
    _ref.invalidate(hikingActivityProvider(id));
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).refresh();
  }

  Future<void> delete(String id) async {
    await _api.delete(id);
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).removeLocal(id);
  }

  static String _wkt(List<LatLng> route) {
    if (route.isEmpty) return 'LINESTRING EMPTY';
    return 'LINESTRING(${route.map((p) => '${p.longitude} ${p.latitude}').join(', ')})';
  }
}
