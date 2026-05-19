import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:turbo/features/search/api.dart';

void main() {
  group('TrailSearchService.findLocationsBy', () {
    test('empty query returns an empty list without hitting the network',
        () async {
      var calls = 0;
      final client = MockClient((_) async {
        calls++;
        return http.Response('{}', 200);
      });
      final service = TrailSearchService(client: client);
      expect(await service.findLocationsBy('   '), isEmpty);
      expect(calls, 0);
    });

    test('sends a WFS GetFeature request with a CQL filter on navn',
        () async {
      Uri? captured;
      final client = MockClient((req) async {
        captured = req.url;
        return http.Response(
          jsonEncode({'type': 'FeatureCollection', 'features': []}),
          200,
        );
      });
      await TrailSearchService(client: client).findLocationsBy('Galdho');
      expect(captured!.host, 'wfs.geonorge.no');
      expect(captured!.queryParameters['REQUEST'], 'GetFeature');
      expect(captured!.queryParameters['TYPENAMES'], 'fotrute');
      expect(captured!.queryParameters['CQL_FILTER'],
          "navn ILIKE '%Galdho%'");
    });

    test('parses LineString features into LocationSearchResult', () async {
      final client = MockClient((_) async => http.Response.bytes(
            utf8.encode(jsonEncode({
              'type': 'FeatureCollection',
              'features': [
                {
                  'properties': {
                    'navn': 'Galdhøpiggen via Spiterstulen',
                    'rutenummer': 'JT-12',
                    'merkemetode': 'Rødmerket',
                  },
                  'geometry': {
                    'type': 'LineString',
                    'coordinates': [
                      [8.31, 61.63],
                      [8.32, 61.64],
                    ],
                  },
                }
              ],
            })),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          ));
      final results = await TrailSearchService(client: client)
          .findLocationsBy('Galdhøpiggen');
      expect(results, hasLength(1));
      expect(results.first.title, 'Galdhøpiggen via Spiterstulen');
      expect(results.first.source, 'turbase');
      expect(results.first.position.latitude, 61.63);
      expect(results.first.position.longitude, 8.31);
      expect(results.first.description, contains('Rødmerket'));
    });

    test('non-200 returns empty list', () async {
      final client =
          MockClient((_) async => http.Response('nope', 503));
      final result = await TrailSearchService(client: client)
          .findLocationsBy('Bergen');
      expect(result, isEmpty);
    });
  });
}
