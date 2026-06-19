import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../models/ntb_poi.dart';
import '../models/ntb_route.dart';
import 'ntb_geojson.dart';

/// Thin REST client for the open Nasjonal Turbase API (`api.nasjonalturbase.no`),
/// the data behind ut.no / DNT. Read-only: it lists places + trips in a viewport
/// and fetches a single trip's full route geometry for the reveal animation.
///
/// Auth is a developer `api_key` query param (not an end-user login). With an
/// empty key the client returns nothing instead of erroring, so the feature is
/// inert-but-safe until a key is configured.
///
/// NOTE: the geo-search parameter ([_geoQuery]) follows the documented
/// Mongo-style `near` form. The exact spelling can vary by API deployment; it
/// is deliberately isolated in one method so it can be adjusted against the
/// live API without touching the rest of the feature.
class NtbClient {
  final http.Client _client;
  final String apiKey;

  static const String host = 'api.nasjonalturbase.no';

  /// API version path segment. Change here if the deployment differs.
  static const String apiVersion = 'v3';

  NtbClient({required this.apiKey, http.Client? client})
      : _client = client ?? http.Client();

  static final Distance _distance = const Distance();

  bool get isConfigured => apiKey.isNotEmpty;

  /// Builds a geo-bounded list query for an object [type] (`steder` / `turer`).
  /// [tags] optionally filters (e.g. `['Hytte']`).
  Uri buildListUri({
    required String type,
    required double minLat,
    required double minLon,
    required double maxLat,
    required double maxLon,
    int limit = 100,
    List<String>? tags,
  }) {
    final centerLat = (minLat + maxLat) / 2;
    final centerLon = (minLon + maxLon) / 2;
    // Radius = half the bbox diagonal, so the search disc covers the viewport.
    final radius = _distance(
          LatLng(minLat, minLon),
          LatLng(maxLat, maxLon),
        ) /
        2;
    final params = <String, String>{
      'api_key': apiKey,
      'limit': '$limit',
      'near': _geoQuery(centerLat, centerLon, radius),
      if (tags != null && tags.isNotEmpty) 'tags': tags.join(','),
    };
    return Uri.https(host, '/$apiVersion/$type', params);
  }

  /// The Mongo-style geographic `near` value: a point plus a max distance in
  /// metres. Isolated so the live-API spelling is a one-line change.
  static String _geoQuery(double lat, double lon, double maxDistanceMeters) {
    return jsonEncode({
      r'$geometry': {
        'type': 'Point',
        // GeoJSON coordinate order is [lon, lat].
        'coordinates': [lon, lat],
      },
      r'$maxDistance': maxDistanceMeters.round(),
    });
  }

  /// Fetches cabins/places ([NtbPoiType.cabin] / [NtbPoiType.place]) and trip
  /// markers ([NtbPoiType.trip]) within the bounds. Network/parse failures are
  /// swallowed (returns whatever succeeded) so a flaky source never crashes the
  /// map.
  Future<List<NtbPoi>> fetchPois({
    required double minLat,
    required double minLon,
    required double maxLat,
    required double maxLon,
    int limitPerType = 80,
  }) async {
    if (!isConfigured) return const [];
    final results = await Future.wait([
      _fetchList(
        type: 'steder',
        minLat: minLat,
        minLon: minLon,
        maxLat: maxLat,
        maxLon: maxLon,
        limit: limitPerType,
        toPoi: poiFromSted,
      ),
      _fetchList(
        type: 'turer',
        minLat: minLat,
        minLon: minLon,
        maxLat: maxLat,
        maxLon: maxLon,
        limit: limitPerType,
        toPoi: poiFromTur,
      ),
    ]);
    return [...results[0], ...results[1]];
  }

  Future<List<NtbPoi>> _fetchList({
    required String type,
    required double minLat,
    required double minLon,
    required double maxLat,
    required double maxLon,
    required int limit,
    required NtbPoi? Function(Map<String, dynamic>) toPoi,
  }) async {
    try {
      final uri = buildListUri(
        type: type,
        minLat: minLat,
        minLon: minLon,
        maxLat: maxLat,
        maxLon: maxLon,
        limit: limit,
      );
      final body = await _getJson(uri);
      final docs = _documents(body);
      final out = <NtbPoi>[];
      for (final doc in docs) {
        final poi = toPoi(doc);
        if (poi != null) out.add(poi);
      }
      return out;
    } catch (_) {
      return const [];
    }
  }

  /// Fetches one `Tur` and projects it to an [NtbRoute] (the polyline + sheet
  /// metadata). Returns `null` on any failure.
  Future<NtbRoute?> fetchRoute(String turId) async {
    if (!isConfigured) return null;
    try {
      final uri = Uri.https(
        host,
        '/$apiVersion/turer/$turId',
        {'api_key': apiKey},
      );
      final body = await _getJson(uri);
      if (body is! Map<String, dynamic>) return null;
      return routeFromTur(body);
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

  /// List responses are either a bare array or `{documents: [...]}` /
  /// `{data: [...]}` depending on endpoint/version. Normalise to a list of maps.
  static List<Map<String, dynamic>> _documents(Object? body) {
    List? list;
    if (body is List) {
      list = body;
    } else if (body is Map) {
      final docs = body['documents'] ?? body['data'] ?? body['results'];
      if (docs is List) list = docs;
    }
    if (list == null) return const [];
    return [
      for (final e in list)
        if (e is Map) Map<String, dynamic>.from(e),
    ];
  }

  // --- document → model projections (static, for unit testing) ---

  static NtbPoi? poiFromSted(Map<String, dynamic> doc) {
    final pos = NtbGeoJson.point(doc['geojson']);
    if (pos == null) return null;
    final tags = _stringList(doc['tags']);
    final isCabin = tags.any((t) => t.toLowerCase() == 'hytte');
    final id = _id(doc);
    return NtbPoi(
      id: id,
      type: isCabin ? NtbPoiType.cabin : NtbPoiType.place,
      title: _title(doc),
      position: pos,
      summary: _summary(doc),
      imageUrl: _firstImage(doc),
      utUrl: _utUrl(doc, isCabin ? 'hytte' : 'sted', id),
    );
  }

  static NtbPoi? poiFromTur(Map<String, dynamic> doc) {
    final pos = NtbGeoJson.point(doc['geojson']);
    if (pos == null) return null;
    final id = _id(doc);
    return NtbPoi(
      id: id,
      type: NtbPoiType.trip,
      title: _title(doc),
      position: pos,
      summary: _summary(doc),
      imageUrl: _firstImage(doc),
      utUrl: _utUrl(doc, 'turforslag', id),
    );
  }

  static NtbRoute routeFromTur(Map<String, dynamic> doc) {
    final id = _id(doc);
    return NtbRoute(
      id: id,
      title: _title(doc),
      points: NtbGeoJson.line(doc['geojson']),
      description: _summary(doc),
      distanceMeters: _distanceMeters(doc['distanse']),
      grade: _string(doc['gradering']),
      imageUrl: _firstImage(doc),
      utUrl: _utUrl(doc, 'turforslag', id),
    );
  }

  // --- field helpers ---

  static String _id(Map<String, dynamic> doc) =>
      (doc['_id'] ?? doc['id'] ?? '').toString();

  static String _title(Map<String, dynamic> doc) {
    final navn = _string(doc['navn']);
    return (navn == null || navn.isEmpty) ? _id(doc) : navn;
  }

  static String? _summary(Map<String, dynamic> doc) {
    for (final key in const ['beskrivelse', 'innledning', 'ingress']) {
      final v = _string(doc[key]);
      if (v != null && v.isNotEmpty) return v;
    }
    return null;
  }

  static String? _string(Object? v) {
    if (v == null) return null;
    final s = v.toString().trim();
    return s.isEmpty ? null : s;
  }

  static List<String> _stringList(Object? v) =>
      v is List ? [for (final e in v) e.toString()] : const [];

  static double? _distanceMeters(Object? v) {
    if (v is num) return v.toDouble();
    if (v is String) return double.tryParse(v);
    return null;
  }

  /// NTB `bilder` entries are image objects (or ids). Pull the first usable URL.
  static String? _firstImage(Map<String, dynamic> doc) {
    final bilder = doc['bilder'];
    if (bilder is! List) return null;
    for (final b in bilder) {
      if (b is String && b.startsWith('http')) return b;
      if (b is Map) {
        for (final key in const ['url', 'original', 'src', 'href']) {
          final v = b[key];
          if (v is String && v.startsWith('http')) return v;
        }
      }
    }
    return null;
  }

  /// Prefer an explicit ut.no link from `lenker`; otherwise construct one from
  /// the document id. (New ut.no numeric ids vs classic NTB ids aren't always
  /// 1:1 — the embedded link is authoritative when present.)
  static String? _utUrl(Map<String, dynamic> doc, String path, String id) {
    final lenker = doc['lenker'];
    if (lenker is List) {
      for (final l in lenker) {
        if (l is Map) {
          final url = l['url'];
          if (url is String && url.contains('ut.no')) return url;
        }
      }
    }
    if (id.isEmpty) return null;
    return 'https://ut.no/$path/$id';
  }
}
