import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

import 'package:turbo/features/weather/api.dart';

/// Builds a real [WeatherFetcher] backed by a [MockClient] that returns
/// the minimum-shaped responses every sub-service expects. Use this in
/// widget tests where the test doesn't care about specific forecast
/// content — only that the weather sheet renders without exploding.
///
/// The four services hit four hosts/paths; the mock client routes by
/// path:
///   - `/weatherapi/locationforecast/...` → empty atmospheric timeseries
///     wrapped in the canonical envelope (200 OK so the service doesn't
///     throw).
///   - `/weatherapi/oceanforecast/...`    → 404 (out of marine coverage)
///   - `/weatherapi/sunrise/...`          → empty `location.time` array
///   - `/weatherapi/metalerts/...`        → empty FeatureCollection
///
/// Why this lives in helpers/ rather than as a per-test stub: the
/// architecture doc forbids mocking internal collaborators
/// (`architecture.context.md` §6.2). The right surface to mock is the
/// HTTP boundary, which is the [http.Client]. Centralising the wiring
/// here keeps every widget test using one canonical setup that survives
/// future changes to `WeatherFetcher`'s shape.
WeatherFetcher buildTestWeatherFetcher() {
  final client = MockClient((request) async {
    final path = request.url.path;
    if (path.contains('/locationforecast/')) {
      return http.Response(
        jsonEncode(const {
          'properties': {'timeseries': []},
        }),
        200,
        headers: const {'content-type': 'application/json; charset=utf-8'},
      );
    }
    if (path.contains('/oceanforecast/')) {
      return http.Response('not in coverage', 404);
    }
    if (path.contains('/sunrise/')) {
      return http.Response(
        jsonEncode(const {
          'location': {'time': []},
        }),
        200,
        headers: const {'content-type': 'application/json; charset=utf-8'},
      );
    }
    if (path.contains('/metalerts/')) {
      return http.Response(
        jsonEncode(const {'type': 'FeatureCollection', 'features': []}),
        200,
        headers: const {'content-type': 'application/json; charset=utf-8'},
      );
    }
    return http.Response('unexpected mock URL: ${request.url}', 404);
  });

  return WeatherFetcher(
    atmospheric: YrAtmosphericService(client: client),
    ocean: YrOceanService(client: client),
    sunrise: YrSunriseService(client: client),
    alerts: MetAlertsService(client: client),
  );
}
