import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';

class HoydedataServiceException implements Exception {
  final int statusCode;
  final String message;
  const HoydedataServiceException(this.statusCode, this.message);
  @override
  String toString() => 'HoydedataServiceException($statusCode): $message';
}

/// Wrapper around Kartverket's Høydedata point-elevation service.
///
/// Endpoint: `ws.geonorge.no/hoydedata/v1/punkt` (single point) and
/// `punkter` (batch). The 1 m DEM covers all of Norway with sub-metre
/// resolution. Outside coverage the service returns NaN per point, which
/// the caller surfaces as a `null` elevation.
class HoydedataService {
  static const String _host = 'ws.geonorge.no';
  static const String _pointPath = '/hoydedata/v1/punkt';

  /// Kartverket's batch endpoint accepts up to ~50 points per request.
  static const int _batchSize = 50;

  /// Minimum gap between consecutive batches. Keeps us under any rate-limit
  /// without explicit backoff plumbing.
  static const Duration _interBatchDelay = Duration(milliseconds: 250);

  final http.Client _client;
  HoydedataService({http.Client? client}) : _client = client ?? http.Client();

  /// Returns one elevation (in metres) per input point, in order. Points
  /// outside Norway resolve to `null`.
  Future<List<double?>> elevationsFor(List<LatLng> points) async {
    if (points.isEmpty) return const [];
    final out = List<double?>.filled(points.length, null);

    for (var start = 0; start < points.length; start += _batchSize) {
      final end = (start + _batchSize).clamp(0, points.length);
      final batch = points.sublist(start, end);
      try {
        final elevations = await _fetchBatch(batch);
        for (var i = 0; i < elevations.length; i++) {
          out[start + i] = elevations[i];
        }
      } on HoydedataServiceException {
        // Skip the failed batch; remaining samples keep their `null`
        // entries. Whole-import failure is surfaced by the caller via the
        // returned list — they can decide whether to keep raw points.
        rethrow;
      }
      if (end < points.length) {
        await Future<void>.delayed(_interBatchDelay);
      }
    }
    return out;
  }

  Future<List<double?>> _fetchBatch(List<LatLng> batch) async {
    // The `/punkt` endpoint accepts a batch via `punkter`, but the value
    // must be a JSON array of `[lon, lat]` pairs — not the
    // semicolon-joined string Kartverket's older docs suggest. Sending
    // the wrong shape returns HTTP 422 ('har ikke gyldig struktur. Det
    // forventes en liste med lister med koordinater, f.eks.
    // [[60,11],[61,12]]').
    final punkter = jsonEncode([
      for (final p in batch)
        [
          double.parse(p.longitude.toStringAsFixed(5)),
          double.parse(p.latitude.toStringAsFixed(5)),
        ],
    ]);
    final uri = Uri.https(_host, _pointPath, {
      'koordsys': '4258',
      'punkter': punkter,
      'geojson': 'false',
    });

    final response = await _client.get(uri, headers: {
      'User-Agent': kTurboUserAgent,
      'Accept': 'application/json',
    });

    if (response.statusCode < 200 || response.statusCode >= 300) {
      throw HoydedataServiceException(
        response.statusCode,
        response.body.isEmpty ? 'Empty body' : response.body,
      );
    }

    final body = utf8.decode(response.bodyBytes);
    return _parse(body, batch.length);
  }

  /// Parse Kartverket's `punkt` response. The schema returns
  /// `{ "punkter": [{ "datakilde": ..., "z": <number|null>, ... }, ...] }`
  /// for batch calls.
  static List<double?> _parse(String body, int expected) {
    final json = jsonDecode(body);
    final out = List<double?>.filled(expected, null);
    if (json is! Map<String, dynamic>) return out;
    final punkter = json['punkter'];
    if (punkter is! List) return out;
    for (var i = 0; i < punkter.length && i < expected; i++) {
      final entry = punkter[i];
      if (entry is! Map<String, dynamic>) continue;
      final z = entry['z'];
      if (z is num) {
        final value = z.toDouble();
        if (value.isFinite && value > -1000 && value < 9000) {
          out[i] = value;
        }
      }
    }
    return out;
  }
}
