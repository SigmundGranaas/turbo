import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/search/api.dart';

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

  /// Reverse-geocoding describeLocation:
  /// Returns a [LocationDescription] with a qualifier so the UI can phrase
  /// "On Galdhøpiggen" / "In Lom" / etc. Never returns "Unknown".
  group('KartverketLocationService.describeLocation', () {
    Map<String, dynamic> mockPoint(String name, String type, double lat,
            double lng,
            {List<Map<String, String>> kommuner = const []}) =>
        {
          'skrivemåte': name,
          'navneobjekttype': type,
          'representasjonspunkt': {'nord': lat, 'øst': lng},
          'kommuner': kommuner,
          'fylker': const [],
          'stedsnummer': 1,
          'stedstatus': 'aktiv',
          'språk': 'nor',
        };

    test('prefers a peak within 100 m and tags it as "on"', () async {
      // Tap is at (61.6360, 8.3120); peak is ~50 m away at (61.6363, 8.3120).
      const tap = LatLng(61.6360, 8.3120);
      final client = MockClient((req) async {
        if (req.url.host != 'ws.geonorge.no') {
          return http.Response('', 404);
        }
        return http.Response(
          jsonEncode({
            'navn': [
              mockPoint('Galdhøpiggen', 'Fjelltopp', 61.6363, 8.3120),
              mockPoint('Spiterhøi', 'Fjelltopp', 61.6900, 8.4300),
              mockPoint('Lom', 'Tettsted', 61.8359, 8.5660),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      expect(d, isNotNull);
      expect(d!.title, 'Galdhøpiggen');
      expect(d.qualifier, LocationQualifier.on);
      expect(d.distanceMeters, lessThan(100));
    });

    test('prefers a peak within 1 km as "close to"', () async {
      // Tap at (61.6360, 8.3120); peak at (61.640, 8.317) ~600 m away.
      const tap = LatLng(61.6360, 8.3120);
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode({
            'navn': [
              mockPoint('Storrist', 'Fjelltopp', 61.6400, 8.3170),
              mockPoint('Lomseggen', 'Ås', 61.6500, 8.4000),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      expect(d!.title, 'Storrist');
      expect(d.qualifier, LocationQualifier.closeTo);
    });

    test('chooses a protected area as "in" when no nearby peak', () async {
      const tap = LatLng(61.500, 8.400);
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode({
            'navn': [
              mockPoint('Jotunheimen', 'Nasjonalpark', 61.500, 8.400),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      expect(d!.title, 'Jotunheimen');
      expect(d.qualifier, LocationQualifier.inArea);
    });

    test('falls back to the kommune lookup when no nearby toponym',
        () async {
      const tap = LatLng(61.840, 8.566);
      var calledKommune = false;
      final client = MockClient((req) async {
        if (req.url.host == 'ws.geonorge.no' &&
            req.url.path.startsWith('/kommuneinfo')) {
          calledKommune = true;
          return http.Response(
            jsonEncode({
              'kommunenavn': 'Lom',
              'kommunenummer': '3434',
              'fylkesnavn': 'Innlandet',
            }),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        // Stedsnavn returns no nearby points.
        return http.Response(
          jsonEncode({'navn': []}),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      expect(d, isNotNull);
      expect(d!.title, 'Lom');
      expect(d.qualifier, LocationQualifier.inArea);
      expect(d.secondary, 'Innlandet');
      expect(calledKommune, isTrue);
    });

    test('skips items with no skrivemåte (no real name) and falls back '
        'to the next-best candidate or the kommune lookup', () async {
      // Reproduces the in-app screenshots: Kartverket returns Gard / Haug
      // features near the coord but with an empty / missing `skrivemåte`,
      // so the parser used to render "Close to Unknown". After the fix we
      // skip nameless items entirely.
      const tap = LatLng(67.22846, 14.49496);
      var kommuneCalled = false;
      final client = MockClient((req) async {
        if (req.url.path.startsWith('/kommuneinfo')) {
          kommuneCalled = true;
          return http.Response(
            jsonEncode({
              'kommunenavn': 'Bodø',
              'kommunenummer': '1804',
              'fylkesnavn': 'Nordland',
            }),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        // Stedsnavn returns two nameless features (a Gard and a Haug)
        // — same shape as the failing screenshots.
        final namelessGard = {
          'navneobjekttype': 'Gard',
          'representasjonspunkt': {
            'nord': tap.latitude + 0.0002,
            'øst': tap.longitude + 0.0002,
          },
          'kommuner': const [],
          'fylker': const [],
          'stedsnummer': 1,
          'stedstatus': 'aktiv',
          'språk': 'nor',
          // No skrivemåte at all.
        };
        final namelessHaug = {
          'skrivemåte': '', // explicitly empty
          'navneobjekttype': 'Haug',
          'representasjonspunkt': {
            'nord': tap.latitude + 0.0003,
            'øst': tap.longitude + 0.0001,
          },
          'kommuner': const [],
          'fylker': const [],
          'stedsnummer': 2,
          'stedstatus': 'aktiv',
          'språk': 'nor',
        };
        return http.Response(
          jsonEncode({'navn': [namelessGard, namelessHaug]}),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      // The bug used to surface "Unknown" here. Now we expect the kommune
      // fallback to kick in (no usable nearby toponym).
      expect(d, isNotNull);
      expect(d!.title, isNot('Unknown'));
      expect(d.title, isNot(''));
      expect(d.title, 'Bodø');
      expect(kommuneCalled, isTrue,
          reason: 'kommune fallback must run when all toponyms are nameless');
    });

    test('prefers a named neighbour over a nameless feature even when the '
        'nameless one is closer', () async {
      const tap = LatLng(61.6360, 8.3120);
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode({
            'navn': [
              // Closer, but no name → must be skipped.
              {
                'navneobjekttype': 'Haug',
                'representasjonspunkt': {'nord': 61.63605, 'øst': 8.31205},
                'kommuner': const [],
                'fylker': const [],
                'stedsnummer': 1,
                'stedstatus': 'aktiv',
                'språk': 'nor',
              },
              // Farther, but named → must win.
              mockPoint('Galdhøpiggen', 'Fjelltopp', 61.6365, 8.3120),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      expect(d!.title, 'Galdhøpiggen');
      expect(d.qualifier, LocationQualifier.on);
    });

    test('rejects items whose skrivemåte is literally "Unknown" or "Ukjent"',
        () async {
      // Defence in depth: even if Kartverket itself returns the literal
      // English/Norwegian word "Unknown" / "Ukjent" as a feature name,
      // the picker must treat it like a nameless entry and fall through.
      const tap = LatLng(67.22846, 14.49496);
      final client = MockClient((req) async {
        if (req.url.path.startsWith('/kommuneinfo')) {
          return http.Response(
            jsonEncode({
              'kommunenavn': 'Bodø',
              'fylkesnavn': 'Nordland',
            }),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        return http.Response(
          jsonEncode({
            'navn': [
              mockPoint('Unknown', 'Gard', tap.latitude, tap.longitude),
              mockPoint('Ukjent', 'Haug',
                  tap.latitude + 0.0001, tap.longitude + 0.0001),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      expect(d!.title.toLowerCase(), isNot('unknown'));
      expect(d.title.toLowerCase(), isNot('ukjent'));
      expect(d.title, 'Bodø');
    });

    test('handles skrivemåte returned as a list of language variants',
        () async {
      const tap = LatLng(69.65, 18.95);
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode({
            'navn': [
              {
                // Future-proofing: the v2-shape `skrivemåte` is a list.
                'skrivemåte': ['Storsteinnes', 'Stuoraviika'],
                'navneobjekttype': 'Tettsted',
                'representasjonspunkt': {
                  'nord': tap.latitude,
                  'øst': tap.longitude,
                },
                'kommuner': const [],
                'fylker': const [],
                'stedsnummer': 1,
                'stedstatus': 'aktiv',
              },
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      expect(d!.title, 'Storsteinnes');
    });

    test('returns null outside Norway when kommune lookup also fails',
        () async {
      // Stedsnavn returns no nearby toponym; kommuneinfo 404s outside Norway.
      final client = MockClient((req) async {
        if (req.url.path.startsWith('/kommuneinfo')) {
          return http.Response('', 404);
        }
        return http.Response(jsonEncode({'navn': []}), 200,
            headers: {'content-type': 'application/json'});
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(const LatLng(51.5, -0.1));
      expect(d, isNull);
    });
  });
}
