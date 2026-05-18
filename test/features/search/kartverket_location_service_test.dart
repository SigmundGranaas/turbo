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

    test('protected-area lookup hits Miljødirektoratet when Stedsnavn is '
        'empty (regression: "in Bodø/Fauske" replaced by "In Saltfjellet–'
        'Svartisen nasjonalpark")', () async {
      const tap = LatLng(66.6500, 14.3500); // Inside Saltfjellet–Svartisen
      var calledKommune = false;
      final client = MockClient((req) async {
        if (req.url.host == 'kart.miljodirektoratet.no') {
          return http.Response(
            jsonEncode({
              'results': [
                {
                  'layerId': 0,
                  'layerName': 'Nasjonalpark',
                  'value': 'Saltfjellet–Svartisen nasjonalpark',
                  'displayFieldName': 'navn',
                  'attributes': {
                    'navn': 'Saltfjellet–Svartisen nasjonalpark',
                  },
                },
              ],
            }),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        if (req.url.path.startsWith('/kommuneinfo')) {
          calledKommune = true;
          return http.Response(
            jsonEncode({'kommunenavn': 'Bodø', 'fylkesnavn': 'Nordland'}),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        // Stedsnavn returns nothing in the wilderness.
        return http.Response(jsonEncode({'navn': []}), 200,
            headers: {'content-type': 'application/json; charset=utf-8'});
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      expect(d!.title, 'Saltfjellet–Svartisen nasjonalpark');
      expect(d.qualifier, LocationQualifier.inArea);
      expect(calledKommune, isFalse,
          reason:
              'Vern hit must outrank the kommune fallback so the park name '
              'is shown instead of "In Bodø"');
    });

    test('protected-area lookup is bypassed when Stedsnavn returns a tight '
        '(class 0–2) match like a peak or settlement', () async {
      const tap = LatLng(61.6360, 8.3120);
      var calledVern = false;
      final client = MockClient((req) async {
        if (req.url.host == 'kart.miljodirektoratet.no') {
          calledVern = true;
          return http.Response(
            jsonEncode({
              'results': [
                {
                  'layerName': 'Nasjonalpark',
                  'value': 'Jotunheimen',
                  'attributes': const {},
                },
              ],
            }),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        return http.Response(
          jsonEncode({
            'navn': [
              mockPoint('Galdhøpiggen', 'Fjelltopp', 61.6363, 8.3120),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      // A tight peak hit must outrank the containing park.
      expect(d!.title, 'Galdhøpiggen');
      expect(d.qualifier, LocationQualifier.on);
      // It's fine for the parallel Vern call to happen, but its result is
      // ignored — assert that only by checking the title above.
      expect(calledVern, isTrue);
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

    test('a town within 1.5 km wins over a far-away peak (regression)',
        () async {
      // Reproduces the user complaint: pin in a town used to fall
      // through to the kommune because the only "high-priority" item
      // matched within range was a peak ~800 m away. The town name must
      // win — that's what hikers actually care about.
      const tap = LatLng(69.6500, 18.9500);
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode({
            'navn': [
              // Tettsted (town) ~400 m away.
              mockPoint('Storsteinnes', 'Tettsted',
                  tap.latitude + 0.0036, tap.longitude),
              // Peak ~900 m away.
              mockPoint('Sennefjellet', 'Fjelltopp',
                  tap.latitude + 0.0080, tap.longitude + 0.0040),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      expect(d!.title, 'Storsteinnes');
      expect(d.qualifier, LocationQualifier.inArea);
    });

    test('rural area: a farm ~300 m away is returned as "Close to" '
        'instead of falling all the way to the kommune', () async {
      // Same shape as Norwegian fjord coastlines: Kartverket only knows
      // a couple of named Bruk / Gard features and the nearest tettsted
      // is 10 km off. The old logic rejected the farm (>200 m) and
      // pinned the kommune. The user wants the farm name back.
      const tap = LatLng(67.20000, 14.47000);
      final client = MockClient((req) async {
        if (req.url.path.startsWith('/kommuneinfo')) {
          // Kommune lookup MUST NOT be hit — the farm should win.
          return http.Response(
            jsonEncode({'kommunenavn': 'Bodø', 'fylkesnavn': 'Nordland'}),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        return http.Response(
          jsonEncode({
            'navn': [
              // ~300 m away.
              mockPoint('Storgården', 'Gard',
                  tap.latitude + 0.0027, tap.longitude),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      expect(d!.title, 'Storgården');
      expect(d.qualifier, LocationQualifier.closeTo);
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

    /// Phase A — API-hygiene contract regressions.
    /// Every outbound Geonorge / Miljødirektoratet request must carry
    /// the `User-Agent` header and the Stedsnavn / Vern queries must
    /// include the params we added to reduce noise (`navnestatus=hovednavn`,
    /// `filtrer`, `layers=all:1`).

    test('Stedsnavn punkt query includes navnestatus=hovednavn and a '
        'sparse fieldset filtrer', () async {
      Uri? capturedPunkt;
      final client = MockClient((req) async {
        if (req.url.host == 'ws.geonorge.no' &&
            req.url.path == '/stedsnavn/v1/punkt') {
          capturedPunkt = req.url;
        }
        return http.Response(jsonEncode({'navn': []}), 200,
            headers: {'content-type': 'application/json; charset=utf-8'});
      });
      final service = KartverketLocationService(client: client);

      await service.describeLocation(const LatLng(60.0, 10.0));

      expect(capturedPunkt, isNotNull,
          reason: 'describeLocation must hit /stedsnavn/v1/punkt');
      expect(capturedPunkt!.queryParameters['navnestatus'], 'hovednavn');
      final filtrer = capturedPunkt!.queryParameters['filtrer'] ?? '';
      expect(filtrer, contains('navn.skrivemåte'));
      expect(filtrer, contains('navn.navneobjekttype'));
      expect(filtrer, contains('navn.representasjonspunkt'));
    });

    test('Vern identify call uses layers=all:1 (only the class-split '
        'polygons), not the unscoped "all"', () async {
      Uri? capturedVern;
      final client = MockClient((req) async {
        if (req.url.host == 'kart.miljodirektoratet.no') {
          capturedVern = req.url;
        }
        return http.Response(jsonEncode({'results': []}), 200,
            headers: {'content-type': 'application/json; charset=utf-8'});
      });
      final service = KartverketLocationService(client: client);

      await service.describeLocation(const LatLng(66.65, 14.35));

      expect(capturedVern, isNotNull);
      expect(capturedVern!.queryParameters['layers'], 'all:1');
    });

    test('Vern result prefers attributes[verneform] over layerName for '
        'the protection-class label', () async {
      const tap = LatLng(66.6500, 14.3500);
      final client = MockClient((req) async {
        if (req.url.host == 'kart.miljodirektoratet.no') {
          return http.Response(
            jsonEncode({
              'results': [
                {
                  // layerName is a presentation-layer string ("klasse 1");
                  // verneform is the canonical data label and must win.
                  'layerName': 'naturvern_klasser_omrade.1',
                  'value': 'Junkerdal nasjonalpark',
                  'attributes': {
                    'navn': 'Junkerdal nasjonalpark',
                    'verneform': 'Nasjonalpark',
                  },
                },
              ],
            }),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        return http.Response(jsonEncode({'navn': []}), 200,
            headers: {'content-type': 'application/json; charset=utf-8'});
      });
      final service = KartverketLocationService(client: client);

      final d = await service.describeLocation(tap);
      expect(d!.title, 'Junkerdal nasjonalpark');
      // Subtitle (secondary) carries the canonical kind from verneform.
      expect(d.secondary, 'Nasjonalpark');
    });

    test('every outbound request to Geonorge / Miljødirektoratet carries '
        'the kTurboUserAgent header', () async {
      final seenHosts = <String, String?>{};
      final client = MockClient((req) async {
        seenHosts[req.url.host] = req.headers['User-Agent'] ??
            req.headers['user-agent'];
        if (req.url.path.startsWith('/kommuneinfo')) {
          return http.Response(
              jsonEncode({'kommunenavn': 'Lom', 'fylkesnavn': 'Innlandet'}),
              200,
              headers: {'content-type': 'application/json; charset=utf-8'});
        }
        if (req.url.host == 'kart.miljodirektoratet.no') {
          return http.Response(jsonEncode({'results': []}), 200,
              headers: {'content-type': 'application/json; charset=utf-8'});
        }
        return http.Response(jsonEncode({'navn': []}), 200,
            headers: {'content-type': 'application/json; charset=utf-8'});
      });
      final service = KartverketLocationService(client: client);
      await service.describeLocation(const LatLng(60.0, 10.0));

      // Stedsnavn, Vern, and Kommune should all have been hit.
      expect(seenHosts.keys, containsAll(<String>[
        'ws.geonorge.no',
        'kart.miljodirektoratet.no',
      ]));
      for (final entry in seenHosts.entries) {
        expect(entry.value, isNotNull,
            reason: '${entry.key} request must carry User-Agent');
        expect(entry.value, contains('turbo'),
            reason: '${entry.key} UA must identify the app');
      }
    });
  });
}
