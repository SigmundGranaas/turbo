import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:turbo/features/external_vector_layers/api.dart';

/// Realistic FeatureCollection shape Geonorge's WFS returns for the
/// friluftsruter2 (Nasjonal turbase) typenames.
///
/// Captured-by-shape rather than verbatim: the `properties` are pruned to
/// the columns the trail-info sheet actually consumes. The geometry coords
/// are real (Galdhøpiggen / Spiterstulen trail) so a regression in
/// lat/lon ordering surfaces here, not just in production.
const _wfsFootpathPayload = {
  'type': 'FeatureCollection',
  'crs': {
    'type': 'name',
    'properties': {'name': 'urn:ogc:def:crs:EPSG::4326'},
  },
  'features': [
    {
      'type': 'Feature',
      'id': 'fotrute.1',
      'geometry': {
        'type': 'LineString',
        'coordinates': [
          [8.412, 61.638],
          [8.415, 61.640],
          [8.420, 61.643],
        ],
      },
      'properties': {
        'navn': 'Galdhøpiggen via Spiterstulen',
        'rutenummer': 'JT-12',
        'merkemetode': 'Rødmerket',
        'vanskelighet': 'Krevende',
        'lengde': 7400,
        'sesong': 'Sommer',
      },
    },
    {
      'type': 'Feature',
      'id': 'fotrute.2',
      'geometry': {
        'type': 'MultiLineString',
        'coordinates': [
          [
            [8.30, 61.60],
            [8.31, 61.61],
          ],
          [
            [8.32, 61.62],
            [8.33, 61.63],
          ],
        ],
      },
      'properties': {
        'navn': 'Besseggen',
        'rutenummer': 'JT-04',
        'merkemetode': 'Rødmerket',
      },
    },
  ],
};

void main() {
  group('Nasjonal turbase WFS (fixture)', () {
    test('parses a realistic Geonorge response into trail polylines',
        () async {
      final client = MockClient((req) async {
        // Sanity: assert we ARE asking the real-looking endpoint.
        expect(req.url.host, 'wfs.geonorge.no');
        expect(req.url.queryParameters['SERVICE'], 'WFS');
        expect(req.url.queryParameters['REQUEST'], 'GetFeature');
        expect(req.url.queryParameters['TYPENAMES'], 'fotrute');
        expect(req.url.queryParameters['OUTPUTFORMAT'], 'application/json');
        return http.Response.bytes(
          utf8.encode(jsonEncode(_wfsFootpathPayload)),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final source = trailVectorSource(TrailSubtype.foot);
      final features = await VectorLayerFetcher(client: client).fetchBounds(
        source,
        minLat: 61.5,
        minLon: 8.1,
        maxLat: 61.8,
        maxLon: 8.6,
      );
      expect(features, hasLength(2));

      final galdhoe = features.first;
      expect(galdhoe.kind, VectorGeometryKind.line);
      expect(galdhoe.rings, hasLength(1));
      // GeoJSON encodes [lon, lat]; our parser must produce LatLng with
      // (lat, lon). A regression would put 8.412 in the latitude slot.
      expect(galdhoe.rings.first.first.latitude, closeTo(61.638, 1e-6));
      expect(galdhoe.rings.first.first.longitude, closeTo(8.412, 1e-6));
      expect(galdhoe.properties['navn'], 'Galdhøpiggen via Spiterstulen');

      final besseggen = features[1];
      expect(besseggen.kind, VectorGeometryKind.line);
      expect(besseggen.rings, hasLength(2),
          reason: 'MultiLineString must produce one ring per part.');
    });

    test('builds the WFS URI we ship with the exact parameter set Geonorge expects',
        () {
      // Snapshot the constructed URI so that if anyone tweaks
      // nasjonal_turbase_source.dart's BBOX axis order or SRSNAME without
      // also revisiting the live test, this assertion fails loudly.
      final uri = trailVectorSource(TrailSubtype.foot).buildUri(
        minLat: 61.5,
        minLon: 8.1,
        maxLat: 61.8,
        maxLon: 8.6,
      );
      expect(uri.host, 'wfs.geonorge.no');
      expect(uri.path, '/skwms1/wfs.friluftsruter2');
      expect(uri.queryParameters, {
        'SERVICE': 'WFS',
        'VERSION': '2.0.0',
        'REQUEST': 'GetFeature',
        'TYPENAMES': 'fotrute',
        'OUTPUTFORMAT': 'application/json',
        'SRSNAME': 'urn:ogc:def:crs:EPSG::4326',
        'BBOX': '61.5,8.1,61.8,8.6,urn:ogc:def:crs:EPSG::4326',
        'COUNT': '300',
      });
    });
  });
}
