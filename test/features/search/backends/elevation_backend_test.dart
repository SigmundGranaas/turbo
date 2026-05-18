import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/search/api.dart';

void main() {
  group('ElevationBackend.elevationAt', () {
    test('parses the elevation from a valid response', () async {
      http.Request? captured;
      final client = MockClient((req) async {
        captured = req;
        return http.Response(
          jsonEncode({
            'punkter': [
              {
                'x': 8.3120,
                'y': 61.6363,
                'z': 2469.0,
                'datakilde': 'DTM1',
              },
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final backend = ElevationBackend(client: client);

      final m = await backend.elevationAt(const LatLng(61.6363, 8.3120));

      expect(m, 2469.0);
      expect(captured, isNotNull);
      expect(captured!.url.host, 'ws.geonorge.no');
      expect(captured!.url.path, '/hoydedata/v1/punkt');
      expect(captured!.url.queryParameters['koordsys'], '4258');
      expect(captured!.headers['User-Agent'], kTurboUserAgent);
    });

    test('returns null when the API returns no punkter', () async {
      final client = MockClient((_) async => http.Response(
            jsonEncode({'punkter': []}),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          ));
      expect(
          await ElevationBackend(client: client)
              .elevationAt(const LatLng(0, 0)),
          isNull);
    });

    test('returns null on non-200 (out of coverage)', () async {
      final client = MockClient((_) async => http.Response('boom', 500));
      expect(
          await ElevationBackend(client: client)
              .elevationAt(const LatLng(0, 0)),
          isNull);
    });

    test('returns null on malformed JSON', () async {
      final client = MockClient(
          (_) async => http.Response('not json at all', 200));
      expect(
          await ElevationBackend(client: client)
              .elevationAt(const LatLng(0, 0)),
          isNull);
    });
  });
}
