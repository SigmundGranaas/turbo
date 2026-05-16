import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:turbo/features/search/data/kartverket_location_service.dart';

void main() {
  group('KartverketLocationService HTTP layer', () {
    test('encodes the query and hits the Stedsnavn endpoint', () async {
      Uri? captured;
      final client = MockClient((req) async {
        captured = req.url;
        return http.Response(jsonEncode({'navn': []}), 200,
            headers: {'content-type': 'application/json; charset=utf-8'});
      });
      final service = KartverketLocationService(client: client);

      await service.findLocationsBy('Oslo fjord');

      expect(captured, isNotNull);
      expect(captured!.host, 'ws.geonorge.no');
      expect(captured!.path, '/stedsnavn/v1/navn');
      // Manual encoding swaps space → %20 (per the source comment); the
      // service appends a trailing `*` to the search term and pins fuzzy + 10.
      expect(captured!.query, contains('sok=Oslo%20fjord*'));
      expect(captured!.query, contains('fuzzy=true'));
      expect(captured!.query, contains('treffPerSide=10'));
    });

    test('parses a single result with full metadata', () async {
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode({
            'navn': [
              {
                'skrivemåte': 'Oslo',
                'navneobjekttype': 'By',
                'representasjonspunkt': {'nord': 59.913, 'øst': 10.752},
                'kommuner': [
                  {'kommunenavn': 'Oslo', 'kommunenummer': '0301'}
                ],
                'fylker': [
                  {'fylkesnavn': 'Oslo', 'fylkesnummer': '03'}
                ],
                'stedsnummer': 1234567,
                'stedstatus': 'aktiv',
                'språk': 'nor',
              }
            ]
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final results = await service.findLocationsBy('oslo');

      expect(results, hasLength(1));
      final r = results.first;
      expect(r.title, 'Oslo');
      expect(r.position.latitude, 59.913);
      expect(r.position.longitude, 10.752);
      expect(r.description, 'By, Oslo, Oslo');
      expect(r.icon, 'city');
      expect(r.source, 'kartverket');
      expect(r.metadata!['stedsnummer'], 1234567);
    });

    test('maps known object types to specific icons', () async {
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode({
            'navn': [
              {
                'skrivemåte': 'Galdhøpiggen',
                'navneobjekttype': 'Fjell',
                'representasjonspunkt': {'nord': 61.6, 'øst': 8.3},
                'kommuner': const [],
                'fylker': const [],
                'stedsnummer': 1,
                'stedstatus': 'aktiv',
                'språk': 'nor',
              },
              {
                'skrivemåte': 'Mjøsa',
                'navneobjekttype': 'Innsjø',
                'representasjonspunkt': {'nord': 60.7, 'øst': 11.0},
                'kommuner': const [],
                'fylker': const [],
                'stedsnummer': 2,
                'stedstatus': 'aktiv',
                'språk': 'nor',
              },
              {
                'skrivemåte': 'Akerselva',
                'navneobjekttype': 'Elv',
                'representasjonspunkt': {'nord': 59.9, 'øst': 10.8},
                'kommuner': const [],
                'fylker': const [],
                'stedsnummer': 3,
                'stedstatus': 'aktiv',
                'språk': 'nor',
              },
            ]
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final r = await service.findLocationsBy('x');
      expect(r.map((e) => e.icon).toList(), ['mountain', 'water', 'water']);
    });

    test('falls back to the "place" icon for unknown object types', () async {
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode({
            'navn': [
              {
                'skrivemåte': 'Test',
                'navneobjekttype': 'Romplass',
                'representasjonspunkt': {'nord': 60.0, 'øst': 10.0},
                'kommuner': const [],
                'fylker': const [],
                'stedsnummer': 99,
                'stedstatus': 'aktiv',
                'språk': 'nor',
              }
            ]
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final r = await service.findLocationsBy('x');
      expect(r.first.icon, 'place');
    });

    test('returns empty list on non-200 responses', () async {
      final client = MockClient((_) async => http.Response('', 503));
      final service = KartverketLocationService(client: client);

      expect(await service.findLocationsBy('oslo'), isEmpty);
    });

    test('returns empty list when the client throws', () async {
      final client = MockClient((_) async => throw Exception('socket fail'));
      final service = KartverketLocationService(client: client);

      // Defensive try/catch in the service swallows the error to keep search
      // resilient. Verified contract.
      expect(await service.findLocationsBy('oslo'), isEmpty);
    });

    test('returns empty list without hitting the network for blank queries',
        () async {
      var called = false;
      final client = MockClient((_) async {
        called = true;
        return http.Response('{}', 200);
      });
      final service = KartverketLocationService(client: client);

      expect(await service.findLocationsBy(''), isEmpty);
      expect(await service.findLocationsBy('   '), isEmpty);
      expect(called, isFalse);
    });

    test('decodes UTF-8 body correctly (Æ Ø Å)', () async {
      // The service does utf8.decode(bodyBytes) explicitly. Verify by sending
      // a Latin-1-looking body that only parses correctly when decoded as UTF-8.
      final body = jsonEncode({
        'navn': [
          {
            'skrivemåte': 'Ærfugløya',
            'navneobjekttype': 'Øy',
            'representasjonspunkt': {'nord': 70.0, 'øst': 21.0},
            'kommuner': const [],
            'fylker': const [],
            'stedsnummer': 7,
            'stedstatus': 'aktiv',
            'språk': 'nor',
          }
        ]
      });
      final client = MockClient((_) async => http.Response.bytes(
            utf8.encode(body),
            200,
            headers: {'content-type': 'application/json'},
          ));
      final service = KartverketLocationService(client: client);

      final r = await service.findLocationsBy('æ');
      expect(r.first.title, 'Ærfugløya');
    });
  });
}
