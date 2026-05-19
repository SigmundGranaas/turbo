import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:http_parser/http_parser.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../models/met_alert.dart';
import 'yr_atmospheric_service.dart' show YrServiceException;

/// Result of a successful MetAlerts fetch at a point or in a viewport.
class MetAlertsResult {
  final List<MetAlert> alerts;
  final DateTime expiresAt;
  final String? lastModified;
  const MetAlertsResult({
    required this.alerts,
    required this.expiresAt,
    required this.lastModified,
  });
}

/// Wrapper around MET Norway's MetAlerts 2.0 endpoint.
///
/// Two query modes are supported:
///  - `currentAtPoint(LatLng)`  — alerts intersecting a single coordinate
///    (used by the marker info sheet to show a contextual banner).
///  - `currentInBounds(...)` — all alerts whose footprint intersects a
///    viewport (used by the map overlay). MetAlerts has no bbox query
///    parameter, so the service fetches the full feature set and filters
///    by bbox client-side. The full set is ~1 MB and changes slowly, so
///    the upstream cache layer absorbs the cost.
class MetAlertsService {
  static const String _host = 'api.met.no';
  static const String _path = '/weatherapi/metalerts/2.0/current.json';
  static const Duration _defaultCacheTtl = Duration(minutes: 10);

  final http.Client _client;

  MetAlertsService({http.Client? client}) : _client = client ?? http.Client();

  Future<MetAlertsResult> currentAtPoint(
    LatLng position, {
    String? ifModifiedSince,
  }) {
    final uri = Uri.https(_host, _path, {
      'lat': position.latitude.toStringAsFixed(4),
      'lon': position.longitude.toStringAsFixed(4),
    });
    return _fetch(uri, ifModifiedSince: ifModifiedSince);
  }

  Future<MetAlertsResult> currentInBounds(
    double minLat,
    double minLon,
    double maxLat,
    double maxLon, {
    String? ifModifiedSince,
  }) async {
    // MetAlerts has no bbox parameter (the API rejects it with HTTP 400
    // — "was not listed in the validation options: bbox"). Fetch the
    // global set and filter client-side by polygon bbox.
    final uri = Uri.https(_host, _path);
    final result = await _fetch(uri, ifModifiedSince: ifModifiedSince);
    final filtered = result.alerts.where((a) {
      if (a.area.isEmpty) return true; // keep area-less alerts (national)
      double minALat = double.infinity, minALon = double.infinity;
      double maxALat = -double.infinity, maxALon = -double.infinity;
      for (final p in a.area) {
        if (p.latitude < minALat) minALat = p.latitude;
        if (p.latitude > maxALat) maxALat = p.latitude;
        if (p.longitude < minALon) minALon = p.longitude;
        if (p.longitude > maxALon) maxALon = p.longitude;
      }
      return !(maxALat < minLat ||
          minALat > maxLat ||
          maxALon < minLon ||
          minALon > maxLon);
    }).toList(growable: false);
    return MetAlertsResult(
      alerts: filtered,
      expiresAt: result.expiresAt,
      lastModified: result.lastModified,
    );
  }

  Future<MetAlertsResult> _fetch(Uri uri, {String? ifModifiedSince}) async {
    final request = http.Request('GET', uri)
      ..headers['User-Agent'] = kTurboUserAgent
      ..headers['Accept'] = 'application/json';
    if (ifModifiedSince != null) {
      request.headers['If-Modified-Since'] = ifModifiedSince;
    }

    final streamed = await _client.send(request);
    final response = await http.Response.fromStream(streamed);

    if (response.statusCode == 304) {
      return MetAlertsResult(
        alerts: const [],
        expiresAt: _readExpires(response.headers),
        lastModified: response.headers['last-modified'],
      );
    }
    if (response.statusCode < 200 || response.statusCode >= 300) {
      throw YrServiceException(
        response.statusCode,
        response.body.isEmpty ? 'Empty body' : response.body,
      );
    }

    final body = utf8.decode(response.bodyBytes);
    final json = jsonDecode(body) as Map<String, dynamic>;
    final features = (json['features'] as List?) ?? const [];

    final alerts = <MetAlert>[];
    for (final raw in features) {
      if (raw is! Map<String, dynamic>) continue;
      final alert = _parseFeature(raw);
      if (alert != null) alerts.add(alert);
    }

    return MetAlertsResult(
      alerts: alerts,
      expiresAt: _readExpires(response.headers),
      lastModified: response.headers['last-modified'],
    );
  }

  static MetAlert? _parseFeature(Map<String, dynamic> feature) {
    final props = feature['properties'] as Map<String, dynamic>?;
    if (props == null) return null;
    final level = MetAlert.parseLevel(props['awareness_level'] as String?);
    if (level == null) return null;

    final id = (props['id'] ?? feature['id'] ?? '').toString();
    final event = (props['event'] ?? props['eventAwarenessName'] ?? '')
        .toString();
    final description = (props['description'] ??
            props['instruction'] ??
            props['title'] ??
            event)
        .toString();

    final onset = _readTime(props['onset'] ?? props['effective']);
    final expires = _readTime(props['expires']);
    if (onset == null || expires == null) return null;

    final area = _extractArea(feature['geometry']);

    return MetAlert(
      id: id,
      level: level,
      event: event,
      description: description,
      onset: onset,
      expires: expires,
      area: area,
    );
  }

  static DateTime? _readTime(Object? raw) {
    if (raw is! String) return null;
    try {
      return DateTime.parse(raw);
    } on FormatException {
      return null;
    }
  }

  /// Pulls out the outermost ring of the first polygon/multipolygon and
  /// converts it to LatLng. Holes and inner rings are dropped — they're
  /// purely cosmetic for the overview overlay this code feeds.
  static List<LatLng> _extractArea(Object? geometry) {
    if (geometry is! Map<String, dynamic>) return const [];
    final type = geometry['type'];
    final coords = geometry['coordinates'];
    if (coords is! List) return const [];

    List<dynamic>? ring;
    if (type == 'Polygon') {
      ring = coords.isNotEmpty ? coords.first as List<dynamic> : null;
    } else if (type == 'MultiPolygon') {
      final first = coords.isNotEmpty ? coords.first : null;
      if (first is List && first.isNotEmpty) {
        ring = first.first as List<dynamic>;
      }
    }
    if (ring == null) return const [];
    return [
      for (final p in ring)
        if (p is List && p.length >= 2)
          LatLng((p[1] as num).toDouble(), (p[0] as num).toDouble()),
    ];
  }

  static DateTime _readExpires(Map<String, String> headers) {
    final raw = headers['expires'];
    if (raw != null) {
      try {
        return parseHttpDate(raw).toUtc();
      } on FormatException {
        // Fall through to default.
      }
    }
    return DateTime.now().toUtc().add(_defaultCacheTtl);
  }
}
