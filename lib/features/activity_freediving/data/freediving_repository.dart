import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:logging/logging.dart';

import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/auth/api.dart';

import '../models/freediving_activity.dart';
import '../models/freediving_conditions_report.dart';
import '../models/freediving_details.dart';
import 'freediving_api.dart';

final freedivingApiProvider = Provider<FreedivingApi>((ref) {
  return FreedivingApi(ref.watch(authenticatedApiClientProvider));
});

final freedivingActivityProvider =
    FutureProvider.family<FreedivingActivity, String>((ref, id) async {
  return activities.fetchActivityCached<FreedivingActivity>(
    ref: ref,
    kindUrlSlug: 'freediving',
    kindKey: 'freediving',
    activityId: id,
    fromJson: FreedivingActivity.fromJson,
  );
});

final freedivingConditionsProvider =
    FutureProvider.family<FreedivingConditionsReport, String>((ref, id) async {
  return activities.fetchConditionsCached<FreedivingConditionsReport>(
    ref: ref,
    kindUrlSlug: 'freediving',
    kindKey: 'freediving',
    activityId: id,
    fromJson: FreedivingConditionsReport.fromJson,
  );
});

final freedivingRepositoryProvider = Provider<FreedivingRepository>((ref) => FreedivingRepository(ref));

class FreedivingRepository {
  final Ref _ref;
  final _log = Logger('FreedivingRepository');
  FreedivingRepository(this._ref);

  FreedivingApi get _api => _ref.read(freedivingApiProvider);

  Future<String> create({
    required String name, String? description,
    required LatLng position, required FreedivingDetails details,
  }) async {
    final id = await _api.create(name: name, description: description, position: position, details: details);
    _log.info('Created freediving activity $id');
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).upsertLocal(
          activities.ActivitySummary(
            id: id, kind: 'freediving', name: name,
            geometry: activities.ActivityGeometry.fromServer(
              wkt: activities.ActivityGeometry.pointWkt(position),
              geometryKind: 'POINT'),
            iconKey: 'freediving', colorHex: '#1565C0',
            updatedAt: DateTime.now().toUtc(), version: 1));
    return id;
  }

  Future<void> update({
    required String id, String? name, String? description,
    LatLng? position, FreedivingDetails? details,
  }) async {
    await _api.update(id: id, name: name, description: description, position: position, details: details);
    _ref.invalidate(freedivingActivityProvider(id));
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).refresh();
  }

  Future<void> delete(String id) async {
    await _api.delete(id);
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).removeLocal(id);
  }
}
