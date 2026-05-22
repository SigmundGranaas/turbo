import 'package:turbo/core/api/api_client.dart';
import '../models/activity_summary.dart';

/// HTTP client for the cross-kind summaries endpoints. Per-kind API
/// services live in each kind feature (e.g.
/// `features/activity_fishing/data/fishing_api.dart`) and exchange typed
/// DTOs.
class ActivitySummariesApi {
  final ApiClient _client;

  ActivitySummariesApi(this._client);

  Future<ActivitySummariesResponse> getByBbox({
    required double minLon,
    required double minLat,
    required double maxLon,
    required double maxLat,
    List<String>? kinds,
  }) async {
    final query = <String, dynamic>{
      'minLon': minLon, 'minLat': minLat,
      'maxLon': maxLon, 'maxLat': maxLat,
    };
    if (kinds != null && kinds.isNotEmpty) {
      query['kinds'] = kinds.join(',');
    }
    final r = await _client.get('/api/activities/summaries/bbox', queryParameters: query);
    final data = r.data as Map<String, dynamic>;
    return ActivitySummariesResponse(
      items: (data['items'] as List)
          .cast<Map<String, dynamic>>()
          .map(ActivitySummary.fromJson)
          .toList(),
      serverTime: DateTime.parse(data['serverTime'] as String),
    );
  }

  Future<ActivitySummariesDelta> getChanges({DateTime? since, int? limit}) async {
    final query = <String, dynamic>{};
    if (since != null) query['since'] = since.toUtc().toIso8601String();
    if (limit != null) query['limit'] = limit;
    final r = await _client.get('/api/activities/summaries/changes', queryParameters: query);
    final data = r.data as Map<String, dynamic>;
    return ActivitySummariesDelta(
      items: (data['items'] as List)
          .cast<Map<String, dynamic>>()
          .map(ActivitySummary.fromJson)
          .toList(),
      deleted: (data['deleted'] as List)
          .cast<Map<String, dynamic>>()
          .map(ActivitySummaryTombstone.fromJson)
          .toList(),
      serverTime: DateTime.parse(data['serverTime'] as String),
    );
  }
}

class ActivitySummariesResponse {
  final List<ActivitySummary> items;
  final DateTime serverTime;
  const ActivitySummariesResponse({required this.items, required this.serverTime});
}

class ActivitySummariesDelta {
  final List<ActivitySummary> items;
  final List<ActivitySummaryTombstone> deleted;
  final DateTime serverTime;
  const ActivitySummariesDelta({
    required this.items,
    required this.deleted,
    required this.serverTime,
  });
}
