import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/api/kartverket_hoydedata_client.dart';
import 'package:turbo/features/saved_paths/api.dart';

/// `HoydedataService` is a thin facade over [KartverketHoydedataClient]
/// living in `core/api/`. The end-to-end HTTP / encoding / parsing tests
/// live in `test/core/api/kartverket_hoydedata_client_test.dart`; this
/// file just proves the facade forwards correctly.
void main() {
  test('HoydedataService.elevationsFor delegates to the shared client',
      () async {
    final client = MockClient((_) async => http.Response(
          jsonEncode({
            'punkter': [
              {'z': 100.0},
              {'z': 200.0},
            ]
          }),
          200,
        ));
    final service =
        HoydedataService(client: KartverketHoydedataClient(client: client));
    final result = await service.elevationsFor([
      const LatLng(60, 5),
      const LatLng(61, 6),
    ]);
    expect(result, [100.0, 200.0]);
  });

  test('HoydedataServiceException is the shared client exception', () {
    // The facade re-exports the exception so existing `on HoydedataServiceException`
    // catches continue to work after the move.
    const e = KartverketHoydedataException(422, 'msg');
    expect(e, isA<HoydedataServiceException>());
  });
}
