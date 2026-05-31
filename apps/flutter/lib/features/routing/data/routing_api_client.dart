import 'package:dio/dio.dart';

import '../models/route_models.dart';

/// Categories of routing failure the UI can branch on without parsing
/// strings.
enum RoutingErrorKind {
  /// Malformed request — fewer than 2 points, unknown preset (HTTP 400).
  badRequest,

  /// The solver found no route — endpoint on water/glacier, no terrain
  /// coverage, or a specific leg failed (HTTP 422). [RoutingException.details]
  /// may carry `{kind, leg_index}` / `{kind, which}`.
  noRoute,

  /// Server error (HTTP 5xx).
  server,

  /// Transport failure — no response (timeout, DNS, connection refused).
  network,
}

/// Thrown by [RoutingApiClient] on any non-success outcome.
class RoutingException implements Exception {
  final RoutingErrorKind kind;
  final String message;

  /// Structured context from the server's error envelope, when present.
  final Map<String, dynamic>? details;

  RoutingException(this.kind, this.message, {this.details});

  @override
  String toString() => 'RoutingException($kind): $message';
}

/// Thin HTTP wrapper over the curated routing API. Stateless; returns
/// typed DTOs and maps every failure onto a [RoutingException]. Holds its
/// own [Dio] against a fixed `baseUrl` (the routing API is public, so it
/// does not need the authenticated app client) — tests inject a mock Dio.
class RoutingApiClient {
  final Dio _dio;

  /// Base URL up to and including the route group, e.g.
  /// `http://localhost:8090/v1/route` (dev, direct) or
  /// `https://api.sandring.no/api/route` (prod, via gateway).
  final String baseUrl;

  RoutingApiClient({required this.baseUrl, Dio? dio}) : _dio = dio ?? Dio();

  // Let 4xx through to typed mapping; only 5xx/transport raise DioException.
  static Options get _opts => Options(
        contentType: 'application/json',
        validateStatus: (s) => s != null && s < 500,
      );

  /// `POST /plan` — solve a route through [req]'s waypoints.
  Future<RoutePlan> plan(RouteRequest req) async {
    try {
      final r = await _dio.post(
        '$baseUrl/plan',
        data: req.toJson(),
        options: _opts,
      );
      if (r.statusCode == 200) {
        return RoutePlan.fromJson(r.data as Map<String, dynamic>);
      }
      throw _httpError(r);
    } on DioException catch (e) {
      throw _dioError(e);
    }
  }

  /// `GET /presets` — list the available trip-style presets.
  Future<List<RoutePreset>> presets() async {
    try {
      final r = await _dio.get('$baseUrl/presets', options: _opts);
      if (r.statusCode == 200) {
        return (r.data as List)
            .map((e) => RoutePreset.fromJson(e as Map<String, dynamic>))
            .toList(growable: false);
      }
      throw _httpError(r);
    } on DioException catch (e) {
      throw _dioError(e);
    }
  }

  RoutingException _httpError(Response r) {
    var message = 'HTTP ${r.statusCode}';
    Map<String, dynamic>? details;
    final data = r.data;
    if (data is Map<String, dynamic>) {
      message = (data['error'] as String?) ?? message;
      details = data['details'] as Map<String, dynamic>?;
    }
    final kind = switch (r.statusCode) {
      400 => RoutingErrorKind.badRequest,
      422 => RoutingErrorKind.noRoute,
      _ => RoutingErrorKind.server,
    };
    return RoutingException(kind, message, details: details);
  }

  RoutingException _dioError(DioException e) {
    final response = e.response;
    if (response != null) return _httpError(response);
    return RoutingException(
      RoutingErrorKind.network,
      e.message ?? 'Network error reaching the routing service',
    );
  }
}
