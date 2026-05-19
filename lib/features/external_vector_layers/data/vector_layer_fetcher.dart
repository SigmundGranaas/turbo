import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../models/vector_feature.dart';
import '../models/vector_layer_source.dart';

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
    if (source.disabled) return const [];
    final uri = source.buildUri(
      minLat: minLat,
      minLon: minLon,
      maxLat: maxLat,
      maxLon: maxLon,
      maxFeatures: maxFeatures,
    );
    final response = await _client.get(uri, headers: {
      'User-Agent': kTurboUserAgent,
      'Accept': 'application/json',
      ...?source.headers,
    });
    if (response.statusCode < 200 || response.statusCode >= 300) {
      throw VectorLayerFetchException(
        response.statusCode,
        response.body.isEmpty ? 'Empty body' : response.body,
      );
    }
    final body = utf8.decode(response.bodyBytes);
    return parseGeoJson(body);
  }

  /// Parses a GeoJSON `FeatureCollection` body into [VectorFeature]s.
  static List<VectorFeature> parseGeoJson(String body) {
    final json = jsonDecode(body);
    if (json is! Map<String, dynamic>) return const [];
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
