import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/saved_paths/api.dart';

void main() {
  group('HoydedataService.elevationsFor', () {
    test('returns elevations in input order with User-Agent attached',
        () async {
      String? userAgent;
      final client = MockClient((req) async {
        userAgent = req.headers['User-Agent'];
        return http.Response(
          jsonEncode({
            'punkter': [
              {'z': 100.5},
              {'z': 250.0},
            ],
          }),
          200,
        );
      });
      final service = HoydedataService(client: client);
      final result = await service.elevationsFor([
        const LatLng(60.0, 5.0),
        const LatLng(60.1, 5.1),
      ]);
      expect(result, [100.5, 250.0]);
      expect(userAgent, kTurboUserAgent);
    });

    test('passes points to ws.geonorge.no/hoydedata/v1/punkt as a JSON '
        'array of [lon, lat] pairs (the only format the API accepts; '
        'semicolon-joined "lat,lon;lat,lon" returns HTTP 422)', () async {
      Uri? captured;
      final client = MockClient((req) async {
        captured = req.url;
        return http.Response(
            jsonEncode({'punkter': [{'z': 50}, {'z': 60}]}), 200);
      });
      await HoydedataService(client: client).elevationsFor(const [
        LatLng(60.0, 5.0),
        LatLng(61.5, 8.25),
      ]);
      expect(captured!.host, 'ws.geonorge.no');
      expect(captured!.path, '/hoydedata/v1/punkt');
      expect(captured!.queryParameters['koordsys'], '4258');

      final raw = captured!.queryParameters['punkter']!;
      final decoded = jsonDecode(raw);
      expect(decoded, isA<List>(),
          reason: '`punkter` must be a JSON array, not a delimited string.');
      expect(decoded, hasLength(2));
      // Kartverket expects [lon, lat] order, matching the GeoJSON axis
      // convention they use everywhere else.
      expect((decoded as List)[0], [5.0, 60.0]);
      expect(decoded[1], [8.25, 61.5]);
      // The old semicolon-joined form would set this — guard against
      // regressions.
      expect(raw, isNot(contains(';')));
    });

    test('returns null where the API reports a non-finite elevation',
        () async {
      final client = MockClient((_) async => http.Response(
            jsonEncode({
              'punkter': [
                {'z': null},
                {'z': 12.3},
              ]
            }),
            200,
          ));
      final result = await HoydedataService(client: client).elevationsFor(
        [const LatLng(0, 0), const LatLng(1, 1)],
      );
      expect(result[0], isNull);
      expect(result[1], 12.3);
    });

    test('throws on 500', () async {
      final client =
          MockClient((_) async => http.Response('boom', 500));
      expect(
        () => HoydedataService(client: client)
            .elevationsFor([const LatLng(60, 5)]),
        throwsA(isA<HoydedataServiceException>()),
      );
    });
  });
}
