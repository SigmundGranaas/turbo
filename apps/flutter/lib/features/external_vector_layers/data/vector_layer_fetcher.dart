import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/gml/gml_to_geojson.dart';
import 'package:turbo/core/util/overpass/overpass_to_geojson.dart';
import 'package:turbo/core/util/user_agent.dart';
import '../models/vector_feature.dart';
import '../models/vector_layer_source.dart';

/// One shared fetcher for every vector source (trails, MetAlerts, …).
/// Stateless — features differ via [VectorLayerSource] descriptors.
final vectorLayerFetcherProvider =
    Provider<VectorLayerFetcher>((_) => VectorLayerFetcher());

class VectorLayerFetchException implements Exception {
  final int statusCode;
  final String message;
  const VectorLayerFetchException(this.statusCode, this.message);
  @override
  String toString() => 'VectorLayerFetchException($statusCode): $message';
}

/// Fetches features from a [VectorLayerSource] for a given WGS84 bounding box.
class VectorLayerFetcher {
  final http.Client _client;
  VectorLayerFetcher({http.Client? client}) : _client = client ?? http.Client();

  Future<List<VectorFeature>> fetchBounds(
    VectorLayerSource source, {
    required double minLat,
    required double minLon,
    required double maxLat,
    required double maxLon,
    int maxFeatures = 200,
  }) async {
    final uri = source.buildUri(
      minLat: minLat,
      minLon: minLon,
      maxLat: maxLat,
      maxLon: maxLon,
      maxFeatures: maxFeatures,
    );
    final response = await _client.get(uri, headers: {
      'User-Agent': kTurboUserAgent,
      // WFS servers vary: some honour Accept, most ignore it and let
      // OUTPUTFORMAT in the URL decide. Send both so a GeoJSON-capable
      // server prefers JSON when given the choice.
      'Accept': 'application/json, text/xml; subtype=gml/3.2.1, application/xml',
      ...?source.headers,
    });
    if (response.statusCode < 200 || response.statusCode >= 300) {
      // response.body would throw for some WFS Content-Type headers
      // (e.g. "text/xml; subtype=gml/3.2.1;charset=UTF-8" trips
      // MediaType.parse). Decode the bytes ourselves.
      final msg = response.bodyBytes.isEmpty
          ? 'Empty body'
          : utf8.decode(response.bodyBytes, allowMalformed: true);
      throw VectorLayerFetchException(response.statusCode, msg);
    }
    final body = utf8.decode(response.bodyBytes, allowMalformed: true);
    return _parse(response.headers['content-type'], body);
  }

  /// Dispatches on body shape (and Content-Type as a hint):
  ///   - XML/GML        → [GmlToGeoJson] → GeoJSON parser
  ///   - Overpass JSON  → [OverpassToGeoJson] → GeoJSON parser
  ///   - GeoJSON        → parsed directly
  ///
  /// Overpass detection: top-level JSON with an `elements` array and no
  /// `features` array — that's the Overpass-API response envelope.
  static List<VectorFeature> _parse(String? contentType, String body) {
    final ct = (contentType ?? '').toLowerCase();
    final looksXml = ct.contains('xml') ||
        ct.contains('gml') ||
        _bodyStartsWith(body, '<');
    if (looksXml) {
      return _parseGeoJsonMap(GmlToGeoJson.convert(body));
    }
    if (_looksOverpass(body)) {
      return _parseGeoJsonMap(OverpassToGeoJson.convert(body));
    }
    return parseGeoJson(body);
  }

  /// Body sniff: Overpass envelopes are `{… "elements": [...] …}` with
  /// no `features` key. We do a partial decode to avoid pulling huge
  /// GeoJSON twice.
  static bool _looksOverpass(String body) {
    try {
      final decoded = jsonDecode(body);
      if (decoded is! Map<String, dynamic>) return false;
      return decoded['elements'] is List && decoded['features'] is! List;
    } on FormatException {
      return false;
    }
  }

  static bool _bodyStartsWith(String body, String prefix) {
    var i = 0;
    while (i < body.length && body.codeUnitAt(i) <= 0x20) {
      i++;
    }
    return body.startsWith(prefix, i);
  }

  /// Parses a GeoJSON `FeatureCollection` body into [VectorFeature]s.
  static List<VectorFeature> parseGeoJson(String body) {
    final json = jsonDecode(body);
    if (json is! Map<String, dynamic>) return const [];
    return _parseGeoJsonMap(json);
  }

  /// Same as [parseGeoJson] but operating on an already-decoded map (used
  /// by the GML path, which produces the same shape natively).
  static List<VectorFeature> _parseGeoJsonMap(Map<String, dynamic> json) {
    final features = json['features'];
    if (features is! List) return const [];
    final out = <VectorFeature>[];
    var counter = 0;
    for (final raw in features) {
      if (raw is! Map<String, dynamic>) continue;
      final geom = raw['geometry'];
      if (geom is! Map<String, dynamic>) continue;
      final type = geom['type'];
      final coords = geom['coordinates'];
      if (coords is! List) continue;

      final id = (raw['id'] ?? 'feat-${counter++}').toString();
      final props = raw['properties'] is Map<String, dynamic>
          ? Map<String, Object?>.from(raw['properties'] as Map)
          : <String, Object?>{};

      switch (type) {
        case 'LineString':
          final ring = _parseRing(coords);
          if (ring.length >= 2) {
            out.add(VectorFeature(
              id: id,
              kind: VectorGeometryKind.line,
              rings: [ring],
              properties: props,
            ));
          }
        case 'MultiLineString':
          final rings = <List<LatLng>>[];
          for (final part in coords) {
            if (part is! List) continue;
            final r = _parseRing(part);
            if (r.length >= 2) rings.add(r);
          }
          if (rings.isNotEmpty) {
            out.add(VectorFeature(
              id: id,
              kind: VectorGeometryKind.line,
              rings: rings,
              properties: props,
            ));
          }
        case 'Polygon':
          if (coords.isNotEmpty) {
            final outer = _parseRing(coords.first as List);
            if (outer.length >= 3) {
              out.add(VectorFeature(
                id: id,
                kind: VectorGeometryKind.polygon,
                rings: [outer],
                properties: props,
              ));
            }
          }
        case 'MultiPolygon':
          final rings = <List<LatLng>>[];
          for (final poly in coords) {
            if (poly is! List || poly.isEmpty) continue;
            final outer = _parseRing(poly.first as List);
            if (outer.length >= 3) rings.add(outer);
          }
          if (rings.isNotEmpty) {
            out.add(VectorFeature(
              id: id,
              kind: VectorGeometryKind.polygon,
              rings: rings,
              properties: props,
            ));
          }
      }
    }
    return out;
  }

  static List<LatLng> _parseRing(List ring) {
    final out = <LatLng>[];
    for (final p in ring) {
      if (p is! List || p.length < 2) continue;
      // GeoJSON: [lon, lat]
      out.add(LatLng((p[1] as num).toDouble(), (p[0] as num).toDouble()));
    }
    return out;
  }
}
