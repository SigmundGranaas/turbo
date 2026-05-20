import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:logging/logging.dart';

import 'package:turbo/features/activities/api.dart' as activities;
import 'package:turbo/features/auth/api.dart';

import '../models/fishing_activity.dart';
import '../models/fishing_conditions_report.dart';
import '../models/fishing_details.dart';
import 'fishing_api.dart';

final fishingApiProvider = Provider<FishingApi>((ref) {
  final apiClient = ref.watch(authenticatedApiClientProvider);
  return FishingApi(apiClient);
});

/// Per-id detail provider. Caches each fishing activity in memory once
/// fetched; rebuilds when the underlying summary changes.
final fishingActivityProvider =
    FutureProvider.family<FishingActivity, String>((ref, id) async {
  final api = ref.watch(fishingApiProvider);
  return api.getById(id);
});

/// Conditions provider — re-fetched on demand (refresh by invalidating).
/// Server caches upstream calls so frequent reloads stay cheap.
final fishingConditionsProvider =
    FutureProvider.family<FishingConditionsReport, String>((ref, id) async {
  return ref.watch(fishingApiProvider).getConditions(id);
});

/// Imperative facade for the fishing kind. Wraps the typed API service
/// and pokes the cross-kind summaries repository on success so the map
/// updates without a delta-sync round-trip.
final fishingRepositoryProvider = Provider<FishingRepository>((ref) {
  return FishingRepository(ref);
});

class FishingRepository {
  final Ref _ref;
  final _log = Logger('FishingRepository');

  FishingRepository(this._ref);

  FishingApi get _api => _ref.read(fishingApiProvider);

  Future<String> create({
    required String name,
    String? description,
    required LatLng position,
    required FishingDetails details,
  }) async {
    final id = await _api.create(
      name: name,
      description: description,
      position: position,
      details: details,
    );
    _log.info('Created fishing activity $id');

    // Optimistic local upsert into the cross-kind summary store; the
    // delta-sync round will reconcile with the server's authoritative
    // version + timestamps.
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).upsertLocal(
          activities.ActivitySummary(
            id: id,
            kind: 'fishing',
            name: name,
            geometry: activities.ActivityGeometry.fromServer(
              wkt: activities.ActivityGeometry.pointWkt(position),
              geometryKind: 'POINT',
            ),
            iconKey: 'fishing',
            colorHex: '#1E6FB8',
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
    LatLng? position,
    FishingDetails? details,
  }) async {
    await _api.update(
      id: id,
      name: name,
      description: description,
      position: position,
      details: details,
    );
    _ref.invalidate(fishingActivityProvider(id));
    // Trigger summaries refresh — the projection might still be
    // propagating; the upsertLocal above keeps the map snappy.
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).refresh();
  }

  Future<void> delete(String id) async {
    await _api.delete(id);
    _ref.read(activities.activitySummariesRepositoryProvider.notifier).removeLocal(id);
  }
}
