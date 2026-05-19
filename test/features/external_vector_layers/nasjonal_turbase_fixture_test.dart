import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:turbo/features/external_vector_layers/api.dart';

/// What a hand-built GeoJSON FeatureCollection looks like — this is the
/// path WMS GetFeatureInfo or any future GeoJSON-capable source would
/// produce. Kept here so a server flipping back to GeoJSON also stays
/// covered by tests.
const _geoJsonFootpathPayload = {
  'type': 'FeatureCollection',
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
        'rutenavn': 'Galdhøpiggen via Spiterstulen',
        'rutenummer': 'JT-12',
        'merking': 'JA',
      },
    },
  ],
};

String _fixture(String name) =>
    File('test/core/util/gml/fixtures/$name').readAsStringSync();

void main() {
  group('Nasjonal turbase WFS — fetcher pipeline', () {
    test('GeoJSON path: a JSON response is parsed by the existing '
        'parseGeoJson code (unchanged from before GML support)', () async {
      final client = MockClient((req) async {
        // Sanity-check the URL targets the canonical WFS.
        expect(req.url.host, 'wfs.geonorge.no');
        expect(req.url.path, '/skwms1/wfs.turogfriluftsruter');
        return http.Response.bytes(
          utf8.encode(jsonEncode(_geoJsonFootpathPayload)),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final features =
          await VectorLayerFetcher(client: client).fetchBounds(
        trailVectorSource(TrailSubtype.foot),
        minLat: 61.5,
        minLon: 8.1,
        maxLat: 61.8,
        maxLon: 8.6,
      );
      expect(features, hasLength(1));
      final f = features.first;
      expect(f.kind, VectorGeometryKind.line);
      expect(f.rings.first.first.latitude, closeTo(61.638, 1e-6));
      expect(f.rings.first.first.longitude, closeTo(8.412, 1e-6));
    });

    test('GML path: a real WFS GML response is run through the converter '
        'first, then through the same VectorFeature parser', () async {
      final client = MockClient((req) async {
        return http.Response.bytes(
          utf8.encode(_fixture('fotrute_one.gml')),
          200,
          // Real Geonorge content-type — note the unusual `subtype=`
          // parameter that breaks `Response.body`'s MediaType parser.
          headers: {
            'content-type': 'text/xml; subtype=gml/3.2.1;charset=UTF-8',
          },
        );
      });
      final features =
          await VectorLayerFetcher(client: client).fetchBounds(
        trailVectorSource(TrailSubtype.foot),
        minLat: 61.5,
        minLon: 8.1,
        maxLat: 61.8,
        maxLon: 8.6,
      );
      expect(features, hasLength(1));
      final f = features.first;
      expect(f.id, 'fotrute.100128');
      expect(f.kind, VectorGeometryKind.line);
      expect(f.rings, hasLength(1));
      // Live fixture's first posList token is `61.673901 8.375147` (lat,
      // lon under urn-form srsName). The converter must normalise to
      // GeoJSON-order and the LatLng must end up with lat=61.67… —
      // not lon=8.37 in the lat slot.
      expect(f.rings.first.first.latitude, closeTo(61.673901, 1e-6));
      expect(f.rings.first.first.longitude, closeTo(8.375147, 1e-6));
      // SOSI property keys reach VectorFeature.properties unchanged.
      expect(f.properties['rutenummer'], isNotNull);
      expect(f.properties['rutenavn'], isNotNull);
    });

    test('GML path also handles three-feature payloads', () async {
      final client = MockClient((_) async => http.Response.bytes(
            utf8.encode(_fixture('fotrute_three.gml')),
            200,
            headers: {
              'content-type': 'text/xml; subtype=gml/3.2.1;charset=UTF-8',
            },
          ));
      final features = await VectorLayerFetcher(client: client).fetchBounds(
        trailVectorSource(TrailSubtype.foot),
        minLat: 61.5,
        minLon: 8.1,
        maxLat: 61.8,
        maxLon: 8.6,
      );
      expect(features, hasLength(3));
      for (final f in features) {
        expect(f.kind, VectorGeometryKind.line);
        expect(f.rings.first.length, greaterThanOrEqualTo(2));
      }
    });

    test('GML path with empty FeatureCollection returns []', () async {
      final client = MockClient((_) async => http.Response.bytes(
            utf8.encode(_fixture('empty.gml')),
            200,
            headers: {
              'content-type': 'text/xml; subtype=gml/3.2.1;charset=UTF-8',
            },
          ));
      final features = await VectorLayerFetcher(client: client).fetchBounds(
        trailVectorSource(TrailSubtype.foot),
        minLat: 0,
        minLon: 0,
        maxLat: 1,
        maxLon: 1,
      );
      expect(features, isEmpty);
    });

    test('content-type sniff: works even when the server omits the header',
        () async {
      // Some misconfigured WFS deployments return XML without a
      // Content-Type. The fetcher should sniff the body's leading
      // character.
      final client = MockClient((_) async => http.Response.bytes(
            utf8.encode(_fixture('fotrute_one.gml')),
            200,
            // intentionally no content-type
          ));
      final features = await VectorLayerFetcher(client: client).fetchBounds(
        trailVectorSource(TrailSubtype.foot),
        minLat: 0,
        minLon: 0,
        maxLat: 1,
        maxLon: 1,
      );
      expect(features, hasLength(1));
    });

    test('buildUri snapshot: canonical Turrutebasen WFS, GML output, urn '
        'SRS, lat,lon BBOX', () {
      final uri = trailVectorSource(TrailSubtype.foot).buildUri(
        minLat: 61.5,
        minLon: 8.1,
        maxLat: 61.8,
        maxLon: 8.6,
      );
      expect(uri.host, 'wfs.geonorge.no');
      expect(uri.path, '/skwms1/wfs.turogfriluftsruter');
      expect(uri.queryParameters, {
        'SERVICE': 'WFS',
        'VERSION': '2.0.0',
        'REQUEST': 'GetFeature',
        'TYPENAMES': 'app:Fotrute',
        'OUTPUTFORMAT': 'text/xml; subtype=gml/3.2.1',
        'SRSNAME': 'urn:ogc:def:crs:EPSG::4326',
        'BBOX': '61.5,8.1,61.8,8.6,urn:ogc:def:crs:EPSG::4326',
        'COUNT': '300',
      });
    });

    test('each subtype produces a distinct id, TYPENAMES and colour', () {
      final foot = trailVectorSource(TrailSubtype.foot);
      final ski = trailVectorSource(TrailSubtype.ski);
      final bike = trailVectorSource(TrailSubtype.bike);
      final other = trailVectorSource(TrailSubtype.other);

      expect({foot.id, ski.id, bike.id, other.id}, hasLength(4));
      expect({foot.color, ski.color, bike.color, other.color}, hasLength(4));

      Map<String, String> queryFor(VectorLayerSource s) => s
          .buildUri(minLat: 60, minLon: 5, maxLat: 60.5, maxLon: 5.5)
          .queryParameters;

      expect(queryFor(foot)['TYPENAMES'], 'app:Fotrute');
      expect(queryFor(ski)['TYPENAMES'], 'app:Skiløype');
      expect(queryFor(bike)['TYPENAMES'], 'app:Sykkelrute');
      expect(queryFor(other)['TYPENAMES'], 'app:AnnenRute');
    });
  });
}
