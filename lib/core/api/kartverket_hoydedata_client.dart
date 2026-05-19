import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';

/// Thrown when the Hoydedata batch endpoint returns an unrecoverable error.
/// Single-point lookups never throw — they collapse failures to `null` for
/// the reverse-geocode enrichment path.
class KartverketHoydedataException implements Exception {
  final int statusCode;
  final String message;
  const KartverketHoydedataException(this.statusCode, this.message);
  @override
  String toString() =>
      'KartverketHoydedataException($statusCode): $message';
}

/// Shared HTTP wrapper around Kartverket's Høydedata point-elevation
/// service (`ws.geonorge.no/hoydedata/v1/punkt`).
///
/// Owns the request shape, the JSON parser, and the rate-limit pacing
/// for batch lookups. Consumers (the reverse-geocode `ElevationBackend`
/// and the saved-paths import backfill) call into one of:
///
///   - [elevationAt] — single point, returns `null` outside coverage or
///     on any failure;
///   - [elevationsFor] — batch, throws on non-2xx so the caller can
///     surface a "backfill failed" hint.
///
/// Lives in `core/api/` because two unrelated features hit the same
/// endpoint; consolidating the parser and the URL contract here keeps
/// them in lockstep and gives us one place to mock for live testing.
class KartverketHoydedataClient {
  static const String _host = 'ws.geonorge.no';
  static const String _path = '/hoydedata/v1/punkt';
  static const Duration _timeout = Duration(seconds: 8);

  /// Kartverket accepts up to ~50 points per batch request.
  static const int batchSize = 50;

  /// Minimum gap between consecutive batches. Keeps us under any
  /// per-second cap without explicit backoff plumbing.
  static const Duration interBatchDelay = Duration(milliseconds: 250);

  final http.Client _client;

  KartverketHoydedataClient({http.Client? client})
      : _client = client ?? http.Client();

  Map<String, String> get _headers => const {
        'Accept': 'application/json',
        'User-Agent': kTurboUserAgent,
      };

  /// Single-point lookup. Failures collapse to `null` so the caller can
  /// silently degrade (used by reverse-geocode enrichment, where missing
  /// elevation just drops the "Galdhøpiggen, 2469 m" suffix).
  Future<double?> elevationAt(LatLng coord) async {
    try {
      final uri = Uri.https(_host, _path, {
        'nord': coord.latitude.toString(),
        'ost': coord.longitude.toString(),
        'koordsys': '4258',
        'geojson': 'false',
      });
      final response = await _client.get(uri, headers: _headers).timeout(
            _timeout,
          );
      if (response.statusCode != 200) return null;
      return _parseSingle(utf8.decode(response.bodyBytes));
    } catch (_) {
      return null;
    }
  }

  /// Batch lookup over [points]. Returns one elevation per input in
  /// order; entries outside Norwegian coverage resolve to `null`.
  ///
  /// Inputs are sliced into [batchSize]-sized chunks, with
  /// [interBatchDelay] between requests. Any chunk that fails with a
  /// non-2xx response throws [KartverketHoydedataException]; the caller
  /// decides whether to keep partial results from earlier chunks.
  Future<List<double?>> elevationsFor(List<LatLng> points) async {
    if (points.isEmpty) return const [];
    final out = List<double?>.filled(points.length, null);
    for (var start = 0; start < points.length; start += batchSize) {
      final end = (start + batchSize).clamp(0, points.length);
      final batch = points.sublist(start, end);
      final elevations = await _fetchBatch(batch);
      for (var i = 0; i < elevations.length; i++) {
        out[start + i] = elevations[i];
      }
      if (end < points.length) {
        await Future<void>.delayed(interBatchDelay);
      }
    }
    return out;
  }

  Future<List<double?>> _fetchBatch(List<LatLng> batch) async {
    // The API requires `punkter` as a JSON-encoded array of [lon, lat]
    // pairs. The earlier "lat,lng;lat,lng" form returned HTTP 422.
    final coords = jsonEncode([
      for (final p in batch) [p.longitude, p.latitude],
    ]);
    final uri = Uri.https(_host, _path, {
      'koordsys': '4258',
      'punkter': coords,
      'geojson': 'false',
    });
    final response = await _client.get(uri, headers: _headers).timeout(
          _timeout,
        );
    if (response.statusCode < 200 || response.statusCode >= 300) {
      throw KartverketHoydedataException(
        response.statusCode,
        response.body.isEmpty ? 'Empty body' : response.body,
      );
    }
    return _parseBatch(utf8.decode(response.bodyBytes), batch.length);
  }

  /// Single-point response shape:
  ///   `{ "punkter": [{ "x": lon, "y": lat, "z": elev, ... }] }`
  static double? _parseSingle(String body) {
    final json = jsonDecode(body);
    if (json is! Map<String, dynamic>) return null;
    final punkter = (json['punkter'] as List?) ?? const [];
    if (punkter.isEmpty) return null;
    final first = punkter.first;
    if (first is! Map<String, dynamic>) return null;
    return _readZ(first['z']);
  }

  /// Batch response shape mirrors the single-point shape but with one
  /// `punkter[]` entry per input point, in input order.
  static List<double?> _parseBatch(String body, int expected) {
    final out = List<double?>.filled(expected, null);
    final json = jsonDecode(body);
    if (json is! Map<String, dynamic>) return out;
    final punkter = json['punkter'];
    if (punkter is! List) return out;
    for (var i = 0; i < punkter.length && i < expected; i++) {
      final entry = punkter[i];
      if (entry is! Map<String, dynamic>) continue;
      out[i] = _readZ(entry['z']);
    }
    return out;
  }

  /// Clips obviously-garbage values. The DEM bottoms out around the
  /// Marianas Trench and tops at ~9 km; anything outside that range is
  /// either a NaN sentinel or a coordinate parsing error upstream.
  static double? _readZ(Object? z) {
    if (z is! num) return null;
    final v = z.toDouble();
    if (!v.isFinite || v <= -1000 || v >= 9000) return null;
    return v;
  }
}

/// Provider for the shared Hoydedata client. Both the search feature
/// (reverse-geocode enrichment) and the saved-paths feature (import
/// elevation backfill) consume this so connection pooling, rate
/// limiting, and test overrides happen in one place.
final kartverketHoydedataClientProvider =
    Provider<KartverketHoydedataClient>((_) => KartverketHoydedataClient());
