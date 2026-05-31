import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/core/api/api_client.dart';
import 'package:turbo/features/auth/api.dart';

import '../models/recommendation_item.dart';
import '../models/today_query.dart';

/// Thin HTTP client for `/api/activities/recommendations`. Returns the
/// typed [RecommendationsResponse]. The Today screen's repository
/// layer wraps this with Riverpod caching + the offline banner.
final todayRecommendationsApiProvider = Provider<TodayRecommendationsApi>((ref) {
  return TodayRecommendationsApi(ref.watch(authenticatedApiClientProvider));
});

class TodayRecommendationsApi {
  final ApiClient _client;
  TodayRecommendationsApi(this._client);

  Future<RecommendationsResponse> fetch(TodayQuery query) async {
    final params = <String, dynamic>{
      'lat': query.location.latitude,
      'lon': query.location.longitude,
      'radiusKm': query.radiusKm,
      'date': query.at.toUtc().toIso8601String(),
    };
    if (query.kinds != null && query.kinds!.isNotEmpty) {
      params['kinds'] = query.kinds!.join(',');
    }
    final r = await _client.get('/api/activities/recommendations', queryParameters: params);
    if (r.statusCode != 200) {
      throw Exception('Recommendations request failed: ${r.statusCode}');
    }
    return RecommendationsResponse.fromJson(r.data as Map<String, dynamic>);
  }
}

/// FutureProvider keyed by [TodayQuery]. Autodispose + a short
/// keepAlive — back-tap into Today should still render instantly after
/// a quick excursion, but stale results don't pile up in memory forever.
final todayRecommendationsProvider = FutureProvider.autoDispose
    .family<RecommendationsResponse, TodayQuery>((ref, query) async {
  final api = ref.read(todayRecommendationsApiProvider);
  final link = ref.keepAlive();
  Future<void>.delayed(const Duration(minutes: 5)).then((_) => link.close());
  return api.fetch(query);
});
