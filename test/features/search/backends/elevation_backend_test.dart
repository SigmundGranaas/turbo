import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/api/kartverket_hoydedata_client.dart';
import 'package:turbo/features/search/api.dart';

/// `ElevationBackend` is a one-method facade over
/// [KartverketHoydedataClient]; the rich HTTP / encoding / parser
/// contract lives in `test/core/api/kartverket_hoydedata_client_test.dart`.
/// These tests prove the facade forwards single-point lookups.
ElevationBackend _backend(http.Client client) =>
    ElevationBackend(client: KartverketHoydedataClient(client: client));

void main() {
  group('ElevationBackend.elevationAt', () {
    test('returns the elevation from the shared client', () async {
      final client = MockClient((_) async => http.Response(
            jsonEncode({
              'punkter': [
                {'x': 8.3120, 'y': 61.6363, 'z': 2469.0}
              ]
            }),
            200,
          ));
      final m = await _backend(client).elevationAt(const LatLng(61.6363, 8.3120));
      expect(m, 2469.0);
    });

    test('null when the API has no coverage', () async {
      final client = MockClient((_) async => http.Response('boom', 500));
      expect(await _backend(client).elevationAt(const LatLng(0, 0)), isNull);
    });
  });
}
