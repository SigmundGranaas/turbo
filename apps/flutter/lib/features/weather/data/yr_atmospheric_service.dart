import 'dart:convert';
import 'dart:developer' as developer;

import 'package:http/http.dart' as http;
import 'package:http_parser/http_parser.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../models/weather_forecast.dart';

/// Thrown when MET's atmospheric endpoint returns an unrecoverable error.
class YrServiceException implements Exception {
  final int statusCode;
  final String message;
  const YrServiceException(this.statusCode, this.message);
  @override
  String toString() => 'YrServiceException($statusCode): $message';
}

/// Result of a successful atmospheric fetch (200 / 203 / 304).
class AtmosphericForecastResult {
  final List<AtmosphericPoint> points;
  final DateTime expiresAt;
  final String? lastModified;

  const AtmosphericForecastResult({
    required this.points,
    required this.expiresAt,
    required this.lastModified,
  });
}

/// Thin wrapper around MET Norway's `locationforecast/2.0/complete` endpoint.
///
/// Following the architecture rule "the service is the public API", this class
/// has no Riverpod wrapper around it — it's exposed via a `Provider<…>` and
/// consumers (the notifier) hold it directly.
class YrAtmosphericService {
  static const String _host = 'api.met.no';
  static const String _path = '/weatherapi/locationforecast/2.0/complete';
  static const Duration _defaultCacheTtl = Duration(minutes: 30);

  final http.Client _client;

  YrAtmosphericService({http.Client? client})
      : _client = client ?? http.Client();

  Future<AtmosphericForecastResult> fetch(
    LatLng position, {
    String? ifModifiedSince,
    AtmosphericForecastResult? previous,
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
      if (previous == null) {
        throw const YrServiceException(304,
            'Received 304 but no previous payload was available to reuse');
      }
      return AtmosphericForecastResult(
        points: previous.points,
        expiresAt: _readExpires(response.headers),
        lastModified: previous.lastModified,
      );
    }

    if (response.statusCode == 203) {
      developer.log(
        'MET Locationforecast returned 203 — endpoint version is deprecated',
        name: 'YrAtmosphericService',
      );
    } else if (response.statusCode < 200 || response.statusCode >= 300) {
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
    final points = [
      for (final entry in timeseries)
        AtmosphericPoint.fromJson(entry as Map<String, dynamic>),
    ];

    return AtmosphericForecastResult(
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
