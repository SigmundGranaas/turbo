import 'dart:convert';

import 'package:dio/dio.dart';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import '../models/route_models.dart';
import 'streaming_http_client.dart';

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

/// An event from the streaming [`RoutingApiClient.planStream`] solve.
sealed class RouteStreamEvent {}

/// A best-path-so-far snapshot — the evolving preview line. Replaces the
/// previous one (latest wins).
class RouteProgress extends RouteStreamEvent {
  final List<LatLng> geometry;
  RouteProgress(this.geometry);
}

/// The final solved route. Terminal success event.
class RouteResult extends RouteStreamEvent {
  final RoutePlan plan;
  RouteResult(this.plan);
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

  /// `POST /plan/stream` — solve [req] and stream the live preview.
  ///
  /// Yields [RouteProgress] (best-path-so-far) repeatedly, then a single
  /// [RouteResult]. Throws [RoutingException] if the server emits an
  /// `error` event or the transport fails.
  Stream<RouteStreamEvent> planStream(RouteRequest req) async* {
    // Uses a streaming-capable client (Fetch API on web; IOClient on
    // mobile) — dio's web adapter buffers and can't deliver SSE frames
    // incrementally.
    final client = makeStreamingClient();
    try {
      final request = http.Request('POST', Uri.parse('$baseUrl/plan/stream'))
        ..headers['Content-Type'] = 'application/json'
        ..headers['Accept'] = 'text/event-stream'
        ..body = jsonEncode(req.toJson());

      final http.StreamedResponse resp;
      try {
        resp = await client.send(request);
      } catch (_) {
        throw RoutingException(
          RoutingErrorKind.network,
          'Could not reach the routing service',
        );
      }
      if (resp.statusCode >= 400) {
        throw _streamHttpError(resp.statusCode, await resp.stream.bytesToString());
      }

      // Parse the SSE byte stream: lines accumulate into (event, data)
      // frames separated by a blank line. `:` keep-alive comments are
      // ignored.
      var buffer = '';
      String? eventName;
      await for (final chunk in resp.stream.transform(utf8.decoder)) {
        buffer += chunk;
        int nl;
        while ((nl = buffer.indexOf('\n')) >= 0) {
          var line = buffer.substring(0, nl);
          buffer = buffer.substring(nl + 1);
          if (line.endsWith('\r')) line = line.substring(0, line.length - 1);
          if (line.isEmpty) {
            eventName = null; // frame boundary
            continue;
          }
          if (line.startsWith('event:')) {
            eventName = line.substring(6).trim();
          } else if (line.startsWith('data:')) {
            final event = _parseFrame(eventName, line.substring(5).trim());
            if (event != null) yield event;
          }
          // ':' comments / unknown fields ignored
        }
      }
    } finally {
      client.close();
    }
  }

  RoutingException _streamHttpError(int status, String body) {
    var message = 'HTTP $status';
    try {
      final data = jsonDecode(body);
      if (data is Map<String, dynamic> && data['error'] is String) {
        message = data['error'] as String;
      }
    } catch (_) {/* non-JSON body */}
    final kind = switch (status) {
      400 => RoutingErrorKind.badRequest,
      422 => RoutingErrorKind.noRoute,
      _ => RoutingErrorKind.server,
    };
    return RoutingException(kind, message);
  }

  RouteStreamEvent? _parseFrame(String? event, String data) {
    switch (event) {
      case 'progress':
        final coords = (jsonDecode(data) as Map<String, dynamic>)['coordinates'] as List;
        return RouteProgress(coords.map((c) {
          final p = c as List;
          return LatLng((p[1] as num).toDouble(), (p[0] as num).toDouble());
        }).toList(growable: false));
      case 'result':
        return RouteResult(RoutePlan.fromJson(jsonDecode(data) as Map<String, dynamic>));
      case 'error':
        final msg = (jsonDecode(data) as Map<String, dynamic>)['error'] as String?;
        throw RoutingException(
          RoutingErrorKind.noRoute,
          msg ?? 'The route could not be solved.',
        );
      default:
        return null;
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
