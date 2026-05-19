import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:turbo/features/search/api.dart';

void main() {
  group('TrailSearchService.findLocationsBy', () {
    // The service is currently a no-op (see the class doc comment): the
    // Geonorge Turrutebasen WFS only exists at `turogfriluftsruter` (not
    // `friluftsruter2`), refuses `application/json` output, and silently
    // ignores `CQL_FILTER`. Until any of those changes, the search source
    // returns no results rather than make broken network calls.

    test('any query returns empty without hitting the network', () async {
      var calls = 0;
      final client = MockClient((_) async {
        calls++;
        return http.Response('{}', 200);
      });
      final service = TrailSearchService(client: client);
      expect(await service.findLocationsBy('Galdho'), isEmpty);
      expect(await service.findLocationsBy('Besseggen'), isEmpty);
      expect(await service.findLocationsBy('   '), isEmpty);
      expect(calls, 0,
          reason: 'The disabled trail search must not issue HTTP calls.');
    });
  });
}
