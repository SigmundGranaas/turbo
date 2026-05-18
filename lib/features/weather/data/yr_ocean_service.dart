import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:http_parser/http_parser.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../models/weather_forecast.dart';
import 'yr_atmospheric_service.dart' show YrServiceException;

/// Result of a successful marine fetch.
class MarineForecastResult {
  final List<MarinePoint> points;
  final DateTime expiresAt;
  final String? lastModified;

  const MarineForecastResult({
    required this.points,
    required this.expiresAt,
    required this.lastModified,
  });
}

/// Thin wrapper around MET Norway's `oceanforecast/2.0/complete` endpoint.
///
/// MET only serves marine data for Nordic seas. Outside that footprint the
/// endpoint returns 4xx or an empty timeseries — both translate to a `null`
/// result here so the caller can silently hide marine UI without an error
/// state.
class YrOceanService {
  static const String _host = 'api.met.no';
  static const String _path = '/weatherapi/oceanforecast/2.0/complete';
  static const Duration _defaultCacheTtl = Duration(minutes: 30);

  final http.Client _client;

  YrOceanService({http.Client? client}) : _client = client ?? http.Client();

  Future<MarineForecastResult?> fetch(
    LatLng position, {
    String? ifModifiedSince,
    MarineForecastResult? previous,
  }) async {
    final uri = Uri.https(_host, _path, {
      'lat': position.latitude.toStringAsFixed(4),
      'lon': position.longitude.toStringAsFixed(4),
    });

    final request = http.Request('GET', uri)
      ..headers['User-Agent'] = kTurboUserAgent
      ..headers['Accept'] = 'application/json';
    if (ifModifiedSince != null) {
      request.headers['If-Modified-Since'] = ifModifiedSince;
    }

    final streamed = await _client.send(request);
    final response = await http.Response.fromStream(streamed);

    if (response.statusCode == 304) {
      if (previous == null) return null;
      return MarineForecastResult(
        points: previous.points,
        expiresAt: _readExpires(response.headers),
        lastModified: previous.lastModified,
      );
    }

    if (response.statusCode == 404 || response.statusCode == 422) {
      // Out of marine coverage. Not an error.
      return null;
    }
    if (response.statusCode >= 500) {
      throw YrServiceException(
        response.statusCode,
        response.body.isEmpty ? 'Empty body' : response.body,
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
    final timeseries = ((json['properties'] as Map<String, dynamic>?)
            ?['timeseries'] as List?) ??
        const [];
    if (timeseries.isEmpty) return null;

    final points = [
      for (final entry in timeseries)
        MarinePoint.fromJson(entry as Map<String, dynamic>),
    ];
    return MarineForecastResult(
      points: points,
      expiresAt: _readExpires(response.headers),
      lastModified: response.headers['last-modified'],
    );
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
