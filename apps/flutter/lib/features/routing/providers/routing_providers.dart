import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/routing_api_client.dart';
import '../data/routing_repository.dart';
import '../models/route_models.dart';

/// Base URL of the routing API, up to and including the route group.
///
/// Mirrors `tileserverBaseUrlProvider` in `curated_paths`: dev talks to
/// the local tileserver directly; release goes through the gateway's
/// `/api/route/*` front door. Override with
/// `--dart-define=TURBO_ROUTING_URL=...` (staging, integration tests).
final routingBaseUrlProvider = Provider<String>((ref) {
  const fromEnv = String.fromEnvironment('TURBO_ROUTING_URL');
  if (fromEnv.isNotEmpty) return fromEnv;
  // Dev: hit the tileserver directly (compose maps 8090) at /v1/route.
  if (kDebugMode) return 'http://localhost:8090/v1/route';
  // Release: through the gateway, which strips /api/route → /v1/route.
  return 'https://api.sandring.no/api/route';
});

final routingApiClientProvider = Provider<RoutingApiClient>((ref) {
  return RoutingApiClient(baseUrl: ref.watch(routingBaseUrlProvider));
});

final routingRepositoryProvider = Provider<RoutingRepository>((ref) {
  return RoutingRepository(ref.watch(routingApiClientProvider));
});

/// The available trip-style presets, fetched once and cached by Riverpod.
final routePresetsProvider = FutureProvider<List<RoutePreset>>((ref) {
  return ref.watch(routingRepositoryProvider).presets();
});
