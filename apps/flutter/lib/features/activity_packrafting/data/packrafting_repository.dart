import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:logging/logging.dart';

import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/auth/api.dart';

import '../models/packrafting_activity.dart';
import '../models/packrafting_conditions_report.dart';
import '../models/packrafting_details.dart';
import 'packrafting_api.dart';

final packraftingApiProvider = Provider<PackraftingApi>((ref) {
  return PackraftingApi(ref.watch(authenticatedApiClientProvider));
});

final packraftingActivityProvider =
    FutureProvider.family<PackraftingActivity, String>((ref, id) async {
  return activities.fetchActivityCached<PackraftingActivity>(
    ref: ref,
    kindUrlSlug: 'packrafting',
    kindKey: 'packrafting',
    activityId: id,
    fromJson: PackraftingActivity.fromJson,
  );
});

final packraftingConditionsProvider =
    FutureProvider.family<PackraftingConditionsReport, String>((ref, id) async {
  return activities.fetchConditionsCached<PackraftingConditionsReport>(
    ref: ref,
    kindUrlSlug: 'packrafting',
    kindKey: 'packrafting',
    activityId: id,
    fromJson: PackraftingConditionsReport.fromJson,
  );
});

/// v2 analysis provider — flow vs user window, DOY-percentile, cold-swim
/// risk drivers, and the kindSlice with current discharge + trend.
final packraftingAnalysisProvider =
    FutureProvider.family<activities.ActivityAnalysis, String>((ref, id) async {
  return activities.fetchAnalysisCached<activities.ActivityAnalysis>(
    ref: ref,
    kindUrlSlug: 'packrafting',
    kindKey: 'packrafting',
    activityId: id,
    fromJson: activities.ActivityAnalysis.fromJson,
  );
});

final packraftingRepositoryProvider = Provider<PackraftingRepository>((ref) => PackraftingRepository(ref));

class PackraftingRepository {
  final Ref _ref;
  final _log = Logger('PackraftingRepository');
  PackraftingRepository(this._ref);

  PackraftingApi get _api => _ref.read(packraftingApiProvider);

  Future<String> create({
    required String name, String? description,
    required List<LatLng> route, required PackraftingDetails details,
  }) async {
    final id = await _api.create(name: name, description: description, route: route, details: details);
    _log.info('Created packrafting activity $id');
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).upsertLocal(
          activities.ActivitySummary(
            id: id, kind: 'packrafting', name: name,
            geometry: activities.ActivityGeometry.fromServer(wkt: _wkt(route), geometryKind: 'LINESTRING'),
            iconKey: 'packrafting', colorHex: '#00838F',
            updatedAt: DateTime.now().toUtc(), version: 1));
    return id;
  }

  Future<void> update({
    required String id, String? name, String? description,
    List<LatLng>? route, PackraftingDetails? details,
  }) async {
    await _api.update(id: id, name: name, description: description, route: route, details: details);
    _ref.invalidate(packraftingActivityProvider(id));
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
