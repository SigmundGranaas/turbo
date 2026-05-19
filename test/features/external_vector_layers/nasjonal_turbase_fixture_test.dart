import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:turbo/features/external_vector_layers/api.dart';

void main() {
  group('Nasjonal turbase vector source', () {
    // The trail vector source is currently disabled — Geonorge's WFS for
    // Turrutebasen lives at `wfs.turogfriluftsruter`, refuses
    // `application/json` output, and only emits GML 3.2.1 which the
    // fetcher can't parse. See the doc on [trailVectorSource]. The WMS
    // tile overlay (rendered separately) continues to show trails on the
    // map.

    test('fetcher short-circuits to an empty list without making a network '
        'call when the source is disabled', () async {
      var calls = 0;
      final client = MockClient((req) async {
        calls++;
        return http.Response('should not be called', 200);
      });
      final source = trailVectorSource(TrailSubtype.foot);
      expect(source.disabled, isTrue,
          reason: 'Trail vector source must stay disabled until the '
              'fetcher learns to parse GML 3.2.1.');
      final features = await VectorLayerFetcher(client: client).fetchBounds(
        source,
        minLat: 61.5,
        minLon: 8.1,
        maxLat: 61.8,
        maxLon: 8.6,
      );
      expect(features, isEmpty);
      expect(calls, 0, reason: 'Disabled source must not hit the network.');
    });

    test('buildUri targets the canonical Geonorge WFS for Turrutebasen — '
        'not the never-existed wfs.friluftsruter2 — so when this source is '
        're-enabled with a GML parser the URL is already right', () {
      final uri = trailVectorSource(TrailSubtype.foot).buildUri(
        minLat: 61.5,
        minLon: 8.1,
        maxLat: 61.8,
        maxLon: 8.6,
      );
      expect(uri.host, 'wfs.geonorge.no');
      // wfs.friluftsruter2 returned "UKJENT APPLIKASJON" — this is the
      // correct path per Kartkatalog metadata for the Turrutebasen dataset.
      expect(uri.path, '/skwms1/wfs.turogfriluftsruter');
      // Format the canonical WFS actually supports.
      expect(
          uri.queryParameters['OUTPUTFORMAT'], 'text/xml; subtype=gml/3.2.1');
      expect(uri.queryParameters['SERVICE'], 'WFS');
      expect(uri.queryParameters['VERSION'], '2.0.0');
      expect(uri.queryParameters['REQUEST'], 'GetFeature');
      expect(uri.queryParameters['BBOX'],
          '61.5,8.1,61.8,8.6,urn:ogc:def:crs:EPSG::4326');
    });
  });
}
