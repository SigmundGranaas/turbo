import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/avalanche_forecast/api.dart';

void main() {
  group('VarsomService.forToday', () {
    test('sends User-Agent and reaches api01.nve.no with lat/lon path',
        () async {
      http.Request? captured;
      final client = MockClient((req) async {
        captured = req;
        return http.Response(jsonEncode([]), 200);
      });
      final svc = VarsomService(client: client);
      await svc.forToday(const LatLng(60.5, 7.0),
          now: DateTime(2026, 1, 15));
      expect(captured!.url.host, 'api01.nve.no');
      expect(captured!.url.path, contains('60.5000/7.0000'));
      expect(captured!.headers['User-Agent'], kTurboUserAgent);
    });

    test('returns null on empty array (outside coverage)', () async {
      final client =
          MockClient((_) async => http.Response(jsonEncode([]), 200));
      final result = await VarsomService(client: client)
          .forToday(const LatLng(60, 5));
      expect(result, isNull);
    });

    test('parses DangerLevel + region + problems', () async {
      final client = MockClient((_) async => http.Response(
            jsonEncode([
              {
                'DangerLevel': 3,
                'RegionId': 3018,
                'RegionName': 'Hardanger',
                'ValidFrom': '2026-01-15T00:00:00',
                'MainText': 'Tricky day in the high terrain.',
                'AvalancheDanger': 'Heightened conditions.',
                'AvalancheProblems': [
                  {
                    'AvalancheProblemTypeName': 'Wind slab',
                    'AvalTriggerSimpleName': 'Easy to trigger',
                    'AvalPropagationName': 'Widespread',
                    'DestructiveSizeExtName': 'Size 2',
                  }
                ],
              }
            ]),
            200,
          ));
      final result = await VarsomService(client: client)
          .forToday(const LatLng(60, 5), now: DateTime(2026, 1, 15));
      expect(result, isNotNull);
      expect(result!.dangerLevel, AvalancheDangerLevel.considerable);
      expect(result.regionName, 'Hardanger');
      expect(result.problems, hasLength(1));
      expect(result.problems.first.typeName, 'Wind slab');
    });

    test('throws on 500', () async {
      final client =
          MockClient((_) async => http.Response('boom', 500));
      expect(
        () => VarsomService(client: client).forToday(const LatLng(60, 5)),
        throwsA(isA<VarsomServiceException>()),
      );
    });
  });
}
