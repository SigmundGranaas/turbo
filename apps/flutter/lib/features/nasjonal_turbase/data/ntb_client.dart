import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../models/ntb_poi.dart';
import '../models/ntb_route.dart';

/// Client for the Turbo backend's Nasjonal Turbase proxy (`/api/places/ntb`).
///
/// The proxy injects the secret api key and normalises ut.no / DNT data into
/// small DTOs server-side, so this client is deliberately thin: it only does
/// HTTP + straight JSON→model mapping, and holds no key. Failures degrade to
/// empty/null so a flaky source never crashes the map.
class NtbClient {
  final http.Client _client;

  /// Backend base URL (e.g. `https://kart-api.sandring.no`).
  final String baseUrl;

  NtbClient({required this.baseUrl, http.Client? client})
      : _client = client ?? http.Client();

  Uri poisUri({
    required double minLat,
    required double minLon,
    required double maxLat,
    required double maxLon,
  }) =>
      Uri.parse('$baseUrl/api/places/ntb/pois').replace(queryParameters: {
        'minLat': '$minLat',
        'minLon': '$minLon',
        'maxLat': '$maxLat',
        'maxLon': '$maxLon',
      });

  Future<List<NtbPoi>> fetchPois({
    required double minLat,
    required double minLon,
    required double maxLat,
    required double maxLon,
  }) async {
    try {
      final body = await _getJson(
          poisUri(minLat: minLat, minLon: minLon, maxLat: maxLat, maxLon: maxLon));
      if (body is! Map<String, dynamic>) return const [];
      final pois = body['pois'];
      if (pois is! List) return const [];
      return [
        for (final p in pois)
          if (p is Map<String, dynamic>) ?poiFromJson(p),
      ];
    } catch (_) {
      return const [];
    }
  }

  Future<NtbRoute?> fetchRoute(String id) async {
    if (id.isEmpty) return null;
    try {
      final body = await _getJson(
          Uri.parse('$baseUrl/api/places/ntb/route/${Uri.encodeComponent(id)}'));
      if (body is! Map<String, dynamic>) return null;
      return routeFromJson(body);
    } catch (_) {
      return null;
    }
  }

  Future<Object?> _getJson(Uri uri) async {
    final res = await _client.get(uri, headers: {
      'User-Agent': kTurboUserAgent,
      'Accept': 'application/json',
    });
    if (res.statusCode < 200 || res.statusCode >= 300) return null;
    final text = utf8.decode(res.bodyBytes, allowMalformed: true);
    if (text.trim().isEmpty) return null;
    return jsonDecode(text);
  }

  // --- DTO → model mapping (static, for unit testing) ---

  static NtbPoi? poiFromJson(Map<String, dynamic> json) {
    final lat = _num(json['lat']);
    final lng = _num(json['lng']);
    if (lat == null || lng == null) return null;
    return NtbPoi(
      id: (json['id'] ?? '').toString(),
      type: _type(json['type']),
      title: (json['title'] ?? '').toString(),
      position: LatLng(lat, lng),
      summary: _str(json['summary']),
      imageUrl: _str(json['imageUrl']),
      utUrl: _str(json['utUrl']),
    );
  }

  static NtbRoute routeFromJson(Map<String, dynamic> json) {
    final rawPoints = json['points'];
    final points = <LatLng>[];
    if (rawPoints is List) {
      for (final p in rawPoints) {
        // Proxy emits GeoJSON order [lng, lat].
        if (p is List && p.length >= 2) {
          final lng = _num(p[0]);
          final lat = _num(p[1]);
          if (lat != null && lng != null) points.add(LatLng(lat, lng));
        }
      }
    }
    return NtbRoute(
      id: (json['id'] ?? '').toString(),
      title: (json['title'] ?? '').toString(),
      points: points,
      description: _str(json['description']),
      distanceMeters: _num(json['distanceMeters']),
      grade: _str(json['grade']),
      imageUrl: _str(json['imageUrl']),
      utUrl: _str(json['utUrl']),
    );
  }

  static NtbPoiType _type(Object? v) => switch (v) {
        'cabin' => NtbPoiType.cabin,
        'trip' => NtbPoiType.trip,
        _ => NtbPoiType.place,
      };

  static double? _num(Object? v) {
    if (v is num) return v.toDouble();
    if (v is String) return double.tryParse(v);
    return null;
  }

  static String? _str(Object? v) {
    if (v == null) return null;
    final s = v.toString().trim();
    return s.isEmpty ? null : s;
  }
}
