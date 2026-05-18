import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/search/api.dart';

void main() {
  group('AddressBackend.nearestAddress', () {
    test('parses the closest address into a LocationDescription', () async {
      http.Request? captured;
      final client = MockClient((req) async {
        captured = req;
        return http.Response(
          jsonEncode({
            'adresser': [
              {
                'adressetekst': 'Storgården 4',
                'postnummer': '2686',
                'poststed': 'LOM',
                'kommunenavn': 'Lom',
              },
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });
      final backend = AddressBackend(client: client);

      final d = await backend.nearestAddress(const LatLng(61.84, 8.57));

      expect(d, isNotNull);
      expect(d!.title, 'Storgården 4');
      expect(d.qualifier, LocationQualifier.near);
      expect(d.secondary, '2686 LOM');
      expect(captured!.url.host, 'ws.geonorge.no');
      expect(captured!.url.path, '/adresser/v1/punktsok');
      expect(captured!.url.queryParameters['radius'], '200');
      expect(captured!.url.queryParameters['treffPerSide'], '1');
      expect(captured!.headers['User-Agent'], kTurboUserAgent);
    });

    test('returns null when no address is within radius', () async {
      final client = MockClient((_) async => http.Response(
            jsonEncode({'adresser': []}),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          ));
      expect(
          await AddressBackend(client: client)
              .nearestAddress(const LatLng(0, 0)),
          isNull);
    });

    test('returns null on non-200', () async {
      final client = MockClient((_) async => http.Response('', 500));
      expect(
          await AddressBackend(client: client)
              .nearestAddress(const LatLng(0, 0)),
          isNull);
    });

    test('returns null when adressetekst is empty', () async {
      final client = MockClient((_) async => http.Response(
            jsonEncode({
              'adresser': [
                {'adressetekst': '', 'postnummer': '0000', 'poststed': '...'}
              ]
            }),
            200,
            headers: {'content-type': 'application/json; charset=utf-8'},
          ));
      expect(
          await AddressBackend(client: client)
              .nearestAddress(const LatLng(0, 0)),
          isNull);
    });
  });
}
