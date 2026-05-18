import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/search/api.dart';

/// Tests for the three-backend orchestrator. Verifies the
/// tier-choice logic ("tight Stedsnavn beats containing park",
/// "park beats kommune", etc.) and the URL/header contracts that
/// the backends each enforce.
///
/// Each backend is constructed with the same `MockClient`; the
/// client routes by URL to simulate the three upstream services.

KartverketReverseGeocoder _geocoder(
  http.Client client, {
  bool withEnrichment = false,
}) {
  return KartverketReverseGeocoder(
    stedsnavn: StedsnavnBackend(client: client),
    protectedArea: ProtectedAreaBackend(client: client),
    kommune: KommuneBackend(client: client),
    address: withEnrichment ? AddressBackend(client: client) : null,
    elevation: withEnrichment ? ElevationBackend(client: client) : null,
  );
}

void main() {
  group('KartverketReverseGeocoder', () {
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
      final d = await _geocoder(client).describe(tap);
      expect(d, isNotNull);
      expect(d!.title, 'Galdhøpiggen');
      expect(d.qualifier, LocationQualifier.on);
      expect(d.distanceMeters, lessThan(100));
    });

    test('prefers a peak within 1 km as "close to"', () async {
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
      final d = await _geocoder(client).describe(tap);
      expect(d!.title, 'Storrist');
      expect(d.qualifier, LocationQualifier.closeTo);
    });

    test('protected-area lookup hits Miljødirektoratet when Stedsnavn is '
        'empty (regression: "in Bodø/Fauske" replaced by the park name)',
        () async {
      const tap = LatLng(66.6500, 14.3500);
      var calledKommune = false;
      final client = MockClient((req) async {
        if (req.url.host == 'kart.miljodirektoratet.no') {
          return http.Response(
            jsonEncode({
              'results': [
                {
                  'layerName': 'Nasjonalpark',
                  'value': 'Saltfjellet–Svartisen nasjonalpark',
                  'attributes': {
                    'navn': 'Saltfjellet–Svartisen nasjonalpark',
                    'verneform': 'Nasjonalpark',
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
        return http.Response(jsonEncode({'navn': []}), 200,
            headers: {'content-type': 'application/json; charset=utf-8'});
      });
      final d = await _geocoder(client).describe(tap);
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
      final client = MockClient((req) async {
        if (req.url.host == 'kart.miljodirektoratet.no') {
          return http.Response(
            jsonEncode({
              'results': [
                {
                  'layerName': 'Nasjonalpark',
                  'value': 'Jotunheimen',
                  'attributes': {'verneform': 'Nasjonalpark'},
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
      final d = await _geocoder(client).describe(tap);
      expect(d!.title, 'Galdhøpiggen');
      expect(d.qualifier, LocationQualifier.on);
    });

    test('falls back to the kommune lookup when no nearby toponym',
        () async {
      const tap = LatLng(61.840, 8.566);
      var calledKommune = false;
      final client = MockClient((req) async {
        if (req.url.path.startsWith('/kommuneinfo')) {
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
        return http.Response(jsonEncode({'navn': []}), 200,
            headers: {'content-type': 'application/json; charset=utf-8'});
      });
      final d = await _geocoder(client).describe(tap);
      expect(d, isNotNull);
      expect(d!.title, 'Lom');
      expect(d.qualifier, LocationQualifier.inArea);
      expect(d.secondary, 'Innlandet');
      expect(calledKommune, isTrue);
    });

    test('skips items with no skrivemåte and falls back to the kommune',
        () async {
      const tap = LatLng(67.22846, 14.49496);
      var kommuneCalled = false;
      final client = MockClient((req) async {
        if (req.url.path.startsWith('/kommuneinfo')) {
          kommuneCalled = true;
          return http.Response(
            jsonEncode({
              'kommunenavn': 'Bodø',
              'fylkesnavn': 'Nordland',
            }),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
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
        };
        final namelessHaug = {
          'skrivemåte': '',
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
      final d = await _geocoder(client).describe(tap);
      expect(d!.title, 'Bodø');
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
              {
                'navneobjekttype': 'Haug',
                'representasjonspunkt': {'nord': 61.63605, 'øst': 8.31205},
                'kommuner': const [],
                'fylker': const [],
                'stedsnummer': 1,
                'stedstatus': 'aktiv',
                'språk': 'nor',
              },
              mockPoint('Galdhøpiggen', 'Fjelltopp', 61.6365, 8.3120),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final d = await _geocoder(client).describe(tap);
      expect(d!.title, 'Galdhøpiggen');
      expect(d.qualifier, LocationQualifier.on);
    });

    test('a town within 1.5 km wins over a far-away peak (regression)',
        () async {
      const tap = LatLng(69.6500, 18.9500);
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode({
            'navn': [
              mockPoint('Storsteinnes', 'Tettsted',
                  tap.latitude + 0.0036, tap.longitude),
              mockPoint('Sennefjellet', 'Fjelltopp',
                  tap.latitude + 0.0080, tap.longitude + 0.0040),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final d = await _geocoder(client).describe(tap);
      expect(d!.title, 'Storsteinnes');
      expect(d.qualifier, LocationQualifier.inArea);
    });

    test('rural area: a farm ~300 m away wins over the kommune fallback',
        () async {
      const tap = LatLng(67.20000, 14.47000);
      final client = MockClient((req) async {
        if (req.url.path.startsWith('/kommuneinfo')) {
          return http.Response(
            jsonEncode({'kommunenavn': 'Bodø', 'fylkesnavn': 'Nordland'}),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        return http.Response(
          jsonEncode({
            'navn': [
              mockPoint('Storgården', 'Gard',
                  tap.latitude + 0.0027, tap.longitude),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final d = await _geocoder(client).describe(tap);
      expect(d!.title, 'Storgården');
      expect(d.qualifier, LocationQualifier.closeTo);
    });

    test('rejects items whose skrivemåte is literally "Unknown" or "Ukjent"',
        () async {
      const tap = LatLng(67.22846, 14.49496);
      final client = MockClient((req) async {
        if (req.url.path.startsWith('/kommuneinfo')) {
          return http.Response(
            jsonEncode({'kommunenavn': 'Bodø', 'fylkesnavn': 'Nordland'}),
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
      final d = await _geocoder(client).describe(tap);
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
      final d = await _geocoder(client).describe(tap);
      expect(d!.title, 'Storsteinnes');
    });

    test('returns null outside Norway when every source is empty',
        () async {
      final client = MockClient((req) async {
        if (req.url.path.startsWith('/kommuneinfo')) {
          return http.Response('', 404);
        }
        return http.Response(jsonEncode({'navn': []}), 200,
            headers: {'content-type': 'application/json'});
      });
      final d = await _geocoder(client).describe(const LatLng(51.5, -0.1));
      expect(d, isNull);
    });

    /// API-hygiene contract regressions.

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
      await _geocoder(client).describe(const LatLng(60.0, 10.0));
      expect(capturedPunkt, isNotNull);
      expect(capturedPunkt!.queryParameters['navnestatus'], 'hovednavn');
      final filtrer = capturedPunkt!.queryParameters['filtrer'] ?? '';
      expect(filtrer, contains('navn.skrivemåte'));
      expect(filtrer, contains('navn.navneobjekttype'));
      expect(filtrer, contains('navn.representasjonspunkt'));
    });

    test('Vern identify call uses layers=all:1 (only the class-split '
        'polygons)', () async {
      Uri? capturedVern;
      final client = MockClient((req) async {
        if (req.url.host == 'kart.miljodirektoratet.no') {
          capturedVern = req.url;
        }
        return http.Response(jsonEncode({'results': []}), 200,
            headers: {'content-type': 'application/json; charset=utf-8'});
      });
      await _geocoder(client).describe(const LatLng(66.65, 14.35));
      expect(capturedVern, isNotNull);
      expect(capturedVern!.queryParameters['layers'], 'all:1');
    });

    test('Vern result prefers attributes[verneform] over layerName for the '
        'protection-class label', () async {
      const tap = LatLng(66.6500, 14.3500);
      final client = MockClient((req) async {
        if (req.url.host == 'kart.miljodirektoratet.no') {
          return http.Response(
            jsonEncode({
              'results': [
                {
                  // layerName is presentation-layer text ("klasse 1");
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
      final d = await _geocoder(client).describe(tap);
      expect(d!.title, 'Junkerdal nasjonalpark');
      expect(d.secondary, 'Nasjonalpark');
    });

    test('every backend request carries the kTurboUserAgent header',
        () async {
      final seenHosts = <String, String?>{};
      final client = MockClient((req) async {
        seenHosts[req.url.host] =
            req.headers['User-Agent'] ?? req.headers['user-agent'];
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
      await _geocoder(client).describe(const LatLng(60.0, 10.0));
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

    /// Enrichment chain: elevation merges into the winning description;
    /// the address backend slots in between protected-area and kommune.

    test('elevation is merged into the winning description', () async {
      const tap = LatLng(61.6363, 8.3120);
      final client = MockClient((req) async {
        if (req.url.path == '/hoydedata/v1/punkt') {
          return http.Response(
              jsonEncode({
                'punkter': [
                  {'x': 8.3120, 'y': 61.6363, 'z': 2469.0}
                ]
              }),
              200,
              headers: {'content-type': 'application/json; charset=utf-8'});
        }
        if (req.url.host == 'kart.miljodirektoratet.no') {
          return http.Response(jsonEncode({'results': []}), 200,
              headers: {'content-type': 'application/json; charset=utf-8'});
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
      final d =
          await _geocoder(client, withEnrichment: true).describe(tap);
      expect(d!.title, 'Galdhøpiggen');
      expect(d.elevationMeters, 2469.0);
    });

    test('address backend wins between protected-area and kommune '
        '(valley populated, no peak, no park)', () async {
      const tap = LatLng(61.840, 8.566);
      var calledKommune = false;
      final client = MockClient((req) async {
        if (req.url.path == '/adresser/v1/punktsok') {
          return http.Response(
            jsonEncode({
              'adresser': [
                {
                  'adressetekst': 'Storgården 4',
                  'postnummer': '2686',
                  'poststed': 'LOM',
                }
              ],
            }),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        if (req.url.path.startsWith('/kommuneinfo')) {
          calledKommune = true;
          return http.Response(
            jsonEncode({'kommunenavn': 'Lom', 'fylkesnavn': 'Innlandet'}),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        return http.Response(jsonEncode({'navn': []}), 200,
            headers: {'content-type': 'application/json; charset=utf-8'});
      });
      final d =
          await _geocoder(client, withEnrichment: true).describe(tap);
      expect(d!.title, 'Storgården 4');
      expect(d.qualifier, LocationQualifier.near);
      expect(d.secondary, '2686 LOM');
      expect(calledKommune, isFalse,
          reason: 'address fallback must outrank the kommune lookup');
    });

    test('elevation enrichment is applied even when the address backend '
        'wins', () async {
      const tap = LatLng(61.840, 8.566);
      final client = MockClient((req) async {
        if (req.url.path == '/hoydedata/v1/punkt') {
          return http.Response(
              jsonEncode({
                'punkter': [
                  {'x': 8.566, 'y': 61.84, 'z': 380.0}
                ]
              }),
              200,
              headers: {'content-type': 'application/json; charset=utf-8'});
        }
        if (req.url.path == '/adresser/v1/punktsok') {
          return http.Response(
            jsonEncode({
              'adresser': [
                {
                  'adressetekst': 'Storgården 4',
                  'postnummer': '2686',
                  'poststed': 'LOM',
                }
              ],
            }),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          );
        }
        return http.Response(jsonEncode({'navn': []}), 200,
            headers: {'content-type': 'application/json; charset=utf-8'});
      });
      final d =
          await _geocoder(client, withEnrichment: true).describe(tap);
      expect(d!.title, 'Storgården 4');
      expect(d.elevationMeters, 380.0);
    });
  });
}
