import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/api/kartverket_hoydedata_client.dart';
import 'package:turbo/core/util/user_agent.dart';

void main() {
  group('KartverketHoydedataClient.elevationAt', () {
    test('returns the z value for a single-point response', () async {
      final client = MockClient((_) async => http.Response(
            jsonEncode({
              'punkter': [
                {'x': 8.3120, 'y': 61.6363, 'z': 2469.0}
              ],
            }),
            200,
          ));
      final result = await KartverketHoydedataClient(client: client)
          .elevationAt(const LatLng(61.6363, 8.3120));
      expect(result, 2469.0);
    });

    test('returns null on non-200', () async {
      final client = MockClient((_) async => http.Response('boom', 500));
      final result = await KartverketHoydedataClient(client: client)
          .elevationAt(const LatLng(0, 0));
      expect(result, isNull);
    });

    test('returns null on malformed JSON', () async {
      final client = MockClient((_) async => http.Response('not json', 200));
      final result = await KartverketHoydedataClient(client: client)
          .elevationAt(const LatLng(0, 0));
      expect(result, isNull);
    });

    test('clips obviously-garbage z values (NaN sentinels, deep-ocean)',
        () async {
      final client = MockClient((_) async => http.Response(
            jsonEncode({
              'punkter': [
                {'z': -9999.0}
              ]
            }),
            200,
          ));
      expect(
          await KartverketHoydedataClient(client: client)
              .elevationAt(const LatLng(0, 0)),
          isNull);
    });
  });

  group('KartverketHoydedataClient.elevationsFor (batch)', () {
    test('encodes punkter as a JSON array of [lon, lat] pairs', () async {
      // Regression: the previous "lat,lng;lat,lng" form returned HTTP 422
      // from Kartverket. The API spec requires JSON-encoded [[lon,lat],…].
      Uri? captured;
      final client = MockClient((req) async {
        captured = req.url;
        return http.Response(
            jsonEncode({
              'punkter': [
                {'z': 100.0},
                {'z': 200.0},
              ]
            }),
            200);
      });
      await KartverketHoydedataClient(client: client).elevationsFor([
        const LatLng(60.0, 5.0),
        const LatLng(61.5, 6.5),
      ]);
      expect(captured!.host, 'ws.geonorge.no');
      expect(captured!.path, '/hoydedata/v1/punkt');
      expect(captured!.queryParameters['koordsys'], '4258');
      final raw = captured!.queryParameters['punkter']!;
      final decoded = jsonDecode(raw);
      expect(decoded, [
        [5.0, 60.0], // lon, lat order
        [6.5, 61.5],
      ]);
    });

    test('returns elevations in input order with User-Agent attached',
        () async {
      String? ua;
      final client = MockClient((req) async {
        ua = req.headers['User-Agent'];
        return http.Response(
            jsonEncode({
              'punkter': [
                {'z': 100.5},
                {'z': 250.0},
              ]
            }),
            200);
      });
      final result = await KartverketHoydedataClient(client: client)
          .elevationsFor([
        const LatLng(60.0, 5.0),
        const LatLng(60.1, 5.1),
      ]);
      expect(result, [100.5, 250.0]);
      expect(ua, kTurboUserAgent);
    });

    test('null entries where the API reports a non-finite elevation',
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
      final result = await KartverketHoydedataClient(client: client)
          .elevationsFor([const LatLng(0, 0), const LatLng(1, 1)]);
      expect(result, [null, 12.3]);
    });

    test('throws KartverketHoydedataException on 5xx', () async {
      final client = MockClient((_) async => http.Response('boom', 500));
      expect(
        () => KartverketHoydedataClient(client: client)
            .elevationsFor([const LatLng(60, 5)]),
        throwsA(isA<KartverketHoydedataException>()),
      );
    });

    test('empty input returns an empty list without firing a request',
        () async {
      var called = false;
      final client = MockClient((_) async {
        called = true;
        return http.Response('{}', 200);
      });
      expect(
          await KartverketHoydedataClient(client: client).elevationsFor([]),
          isEmpty);
      expect(called, isFalse);
    });
  });
}
