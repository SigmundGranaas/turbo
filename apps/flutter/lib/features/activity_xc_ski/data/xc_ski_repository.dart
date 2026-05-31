import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:logging/logging.dart';

import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/auth/api.dart';

import '../models/xc_ski_activity.dart';
import '../models/xc_ski_conditions_report.dart';
import '../models/xc_ski_details.dart';
import 'xc_ski_api.dart';

final xcSkiApiProvider = Provider<XcSkiApi>((ref) {
  final apiClient = ref.watch(authenticatedApiClientProvider);
  return XcSkiApi(apiClient);
});

final xcSkiActivityProvider = FutureProvider.family<XcSkiActivity, String>((ref, id) async {
  return activities.fetchActivityCached<XcSkiActivity>(
    ref: ref,
    kindUrlSlug: 'xc-ski',
    kindKey: 'xc_ski',
    activityId: id,
    fromJson: XcSkiActivity.fromJson,
  );
});

final xcSkiConditionsProvider =
    FutureProvider.family<XcSkiConditionsReport, String>((ref, id) async {
  return activities.fetchConditionsCached<XcSkiConditionsReport>(
    ref: ref,
    kindUrlSlug: 'xc-ski',
    kindKey: 'xc_ski',
    activityId: id,
    fromJson: XcSkiConditionsReport.fromJson,
  );
});

/// v2 analysis provider. Hits `/api/activities/xc-ski/{id}/analysis` and
/// falls back to the cached payload offline. Used by
/// `XcSkiConditionsPanel` to render the new analysis surface (drivers +
/// windows + warnings + provenance) instead of the legacy report.
final xcSkiAnalysisProvider =
    FutureProvider.family<activities.ActivityAnalysis, String>((ref, id) async {
  return activities.fetchAnalysisCached<activities.ActivityAnalysis>(
    ref: ref,
    kindUrlSlug: 'xc-ski',
    kindKey: 'xc_ski',
    activityId: id,
    fromJson: activities.ActivityAnalysis.fromJson,
  );
});

final xcSkiRepositoryProvider = Provider<XcSkiRepository>((ref) => XcSkiRepository(ref));

class XcSkiRepository {
  final Ref _ref;
  final _log = Logger('XcSkiRepository');
  XcSkiRepository(this._ref);

  XcSkiApi get _api => _ref.read(xcSkiApiProvider);

  Future<String> create({
    required String name, String? description,
    required List<LatLng> route, required XcSkiDetails details,
  }) async {
    final id = await _api.create(name: name, description: description, route: route, details: details);
    _log.info('Created xc ski activity $id');
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).upsertLocal(
          activities.ActivitySummary(
            id: id, kind: 'xc_ski', name: name,
            geometry: activities.ActivityGeometry.fromServer(wkt: _wkt(route), geometryKind: 'LINESTRING'),
            iconKey: 'xc_ski', colorHex: '#0288D1',
            updatedAt: DateTime.now().toUtc(), version: 1));
    return id;
  }

  Future<void> update({
    required String id, String? name, String? description,
    List<LatLng>? route, XcSkiDetails? details,
  }) async {
    await _api.update(id: id, name: name, description: description, route: route, details: details);
    _ref.invalidate(xcSkiActivityProvider(id));
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
