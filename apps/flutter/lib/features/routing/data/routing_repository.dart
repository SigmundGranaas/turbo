import 'package:latlong2/latlong.dart';

import '../models/route_models.dart';
import 'routing_api_client.dart';

/// The feature's stable data surface for the UI: plan routes and list
/// presets. Thin over [RoutingApiClient] today — kept as the seam where
/// caching, retries, or offline behaviour land later without touching
/// callers.
class RoutingRepository {
  final RoutingApiClient _client;

  RoutingRepository(this._client);

  /// Plan a route through [points] (start, vias, end). Throws
  /// [RoutingException] on failure.
  Future<RoutePlan> plan({
    required List<LatLng> points,
    String? preset,
    String? profile,
  }) =>
      _client.plan(RouteRequest(
        points: points,
        preset: preset,
        profile: profile,
      ));

  /// Plan a route, streaming the live preview ([RouteProgress]) and the
  /// final [RouteResult]. Throws [RoutingException] on failure.
  Stream<RouteStreamEvent> planStream({
    required List<LatLng> points,
    String? preset,
    String? profile,
  }) =>
      _client.planStream(RouteRequest(
        points: points,
        preset: preset,
        profile: profile,
      ));

  /// List the available trip-style presets.
  Future<List<RoutePreset>> presets() => _client.presets();
}
