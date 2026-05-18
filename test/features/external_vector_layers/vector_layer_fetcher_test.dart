import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/external_vector_layers/api.dart';

VectorLayerSource _source({
  required String id,
  Uri Function({
    required double minLat,
    required double minLon,
    required double maxLat,
    required double maxLon,
    int? maxFeatures,
  })? uri,
}) =>
    VectorLayerSource(
      id: id,
      name: (_) => id,
      buildUri: uri ??
          (({
            required minLat,
            required minLon,
            required maxLat,
            required maxLon,
            maxFeatures,
          }) =>
              Uri.https('example.com', '/wfs', {'bbox': '$minLon,$minLat'})),
    );

void main() {
  group('VectorLayerFetcher.fetchBounds', () {
    test('attaches User-Agent and calls source.buildUri with the bbox',
        () async {
      Uri? captured;
      final source = _source(
        id: 'trails',
        uri: ({
          required minLat,
          required minLon,
          required maxLat,
          required maxLon,
          maxFeatures,
        }) {
          captured = Uri.https('wfs.example', '/get', {
            'bbox': '$minLat,$minLon,$maxLat,$maxLon',
            'count': '${maxFeatures ?? 200}',
          });
          return captured!;
        },
      );
      String? userAgent;
      final client = MockClient((req) async {
        userAgent = req.headers['User-Agent'];
        return http.Response(
          jsonEncode({'type': 'FeatureCollection', 'features': []}),
          200,
        );
      });
      await VectorLayerFetcher(client: client).fetchBounds(
        source,
        minLat: 59.5,
        minLon: 5.0,
        maxLat: 60.0,
        maxLon: 5.5,
      );
      expect(userAgent, kTurboUserAgent);
      expect(captured!.queryParameters['bbox'], '59.5,5.0,60.0,5.5');
    });

    test('throws on non-2xx', () async {
      final client =
          MockClient((_) async => http.Response('nope', 503));
      expect(
        () => VectorLayerFetcher(client: client).fetchBounds(
          _source(id: 'x'),
          minLat: 0,
          minLon: 0,
          maxLat: 1,
          maxLon: 1,
        ),
        throwsA(isA<VectorLayerFetchException>()),
      );
    });
  });

  group('VectorLayerFetcher.parseGeoJson', () {
    test('parses LineString into a single line feature with one ring',
        () {
      final features = VectorLayerFetcher.parseGeoJson(jsonEncode({
        'type': 'FeatureCollection',
        'features': [
          {
            'type': 'Feature',
            'id': 'a',
            'properties': {'navn': 'Bergen-trail'},
            'geometry': {
              'type': 'LineString',
              'coordinates': [
                [5.32, 60.39],
                [5.40, 60.41],
              ],
            },
          }
        ],
      }));
      expect(features, hasLength(1));
      expect(features.first.kind, VectorGeometryKind.line);
      expect(features.first.rings, hasLength(1));
      expect(features.first.rings.first.first.longitude, 5.32);
      expect(features.first.rings.first.first.latitude, 60.39);
      expect(features.first.properties['navn'], 'Bergen-trail');
    });

    test('parses MultiLineString into a single feature with multiple rings',
        () {
      final features = VectorLayerFetcher.parseGeoJson(jsonEncode({
        'type': 'FeatureCollection',
        'features': [
          {
            'type': 'Feature',
            'properties': {},
            'geometry': {
              'type': 'MultiLineString',
              'coordinates': [
                [
                  [5.0, 60.0],
                  [5.1, 60.1],
                ],
                [
                  [5.5, 60.5],
                  [5.6, 60.6],
                ],
              ],
            },
          }
        ],
      }));
      expect(features.single.rings, hasLength(2));
    });

    test('parses Polygon (uses outer ring only)', () {
      final features = VectorLayerFetcher.parseGeoJson(jsonEncode({
        'type': 'FeatureCollection',
        'features': [
          {
            'type': 'Feature',
            'properties': {},
            'geometry': {
              'type': 'Polygon',
              'coordinates': [
                [
                  [5.0, 60.0],
                  [5.1, 60.0],
                  [5.1, 60.1],
                  [5.0, 60.1],
                  [5.0, 60.0],
                ],
                // Inner ring (hole) — should be dropped.
                [
                  [5.05, 60.05],
                  [5.07, 60.05],
                  [5.07, 60.07],
                  [5.05, 60.05],
                ],
              ],
            },
          }
        ],
      }));
      expect(features.single.kind, VectorGeometryKind.polygon);
      expect(features.single.rings.single, hasLength(5));
    });

    test('drops invalid features without exploding', () {
      final features = VectorLayerFetcher.parseGeoJson(jsonEncode({
        'type': 'FeatureCollection',
        'features': [
          {'type': 'Feature', 'geometry': null},
          {'type': 'Feature', 'geometry': {'type': 'Point', 'coordinates': [5, 60]}},
        ],
      }));
      expect(features, isEmpty);
    });
  });

  group('VectorLayerCache', () {
    test('returns null for unknown keys', () {
      expect(VectorLayerCache().get('x'), isNull);
    });

    test('keeps recently used entries; evicts oldest when over capacity', () {
      final cache = VectorLayerCache(capacity: 2);
      cache.put('a', const []);
      cache.put('b', const []);
      cache.get('a');
      cache.put('c', const []);
      expect(cache.size, 2);
      expect(cache.get('a'), isNotNull);
      expect(cache.get('b'), isNull);
      expect(cache.get('c'), isNotNull);
    });
  });

  // Ensure the trail helper builds a real WFS-ish URI.
  test('nasjonalTurbaseVectorSource builds a WFS URI with a bbox', () {
    WidgetsFlutterBinding.ensureInitialized();
    final source = nasjonalTurbaseVectorSource();
    final uri = source.buildUri(
      minLat: 60.0,
      minLon: 5.0,
      maxLat: 60.5,
      maxLon: 5.5,
    );
    expect(uri.host, 'wfs.geonorge.no');
    expect(uri.queryParameters['REQUEST'], 'GetFeature');
    expect(uri.queryParameters['BBOX'], contains('60.0,5.0,60.5,5.5'));
  });
}
