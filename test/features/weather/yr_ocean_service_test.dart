import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:http_parser/http_parser.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/weather/api.dart';
import 'package:turbo/features/weather/data/yr_ocean_service.dart';

const _samplePayload = {
  'properties': {
    'timeseries': [
      {
        'time': '2026-05-17T12:00:00Z',
        'data': {
          'instant': {
            'details': {
              'sea_surface_wave_height': 1.4,
              'sea_surface_wave_from_direction': 280.0,
              'sea_water_temperature': 11.3,
              'sea_water_speed': 0.2,
            },
          },
        },
      }
    ],
  },
};

void main() {
  group('YrOceanService.fetch', () {
    test('hits the oceanforecast endpoint with required headers', () async {
      http.Request? captured;
      final client = MockClient((req) async {
        captured = req;
        return http.Response(jsonEncode(_samplePayload), 200, headers: {
          'content-type': 'application/json; charset=utf-8',
          'expires': 'Sun, 17 May 2026 12:25:00 GMT',
        });
      });
      await YrOceanService(client: client).fetch(const LatLng(60.39, 5.32));
      expect(captured!.url.host, 'api.met.no');
      expect(captured!.url.path, '/weatherapi/oceanforecast/2.0/complete');
      expect(captured!.url.queryParameters['lat'], '60.3900');
      expect(captured!.url.queryParameters['lon'], '5.3200');
      expect(captured!.headers['User-Agent'], kTurboUserAgent);
      expect(captured!.headers['Accept'], 'application/json');
    });

    test('parses 200 response into MarineForecastResult', () async {
      final client = MockClient((_) async {
        return http.Response(jsonEncode(_samplePayload), 200, headers: {
          'content-type': 'application/json; charset=utf-8',
          'last-modified': 'Sun, 17 May 2026 11:55:00 GMT',
          'expires': 'Sun, 17 May 2026 12:25:00 GMT',
        });
      });
      final result =
          await YrOceanService(client: client).fetch(const LatLng(60, 5));
      expect(result, isNotNull);
      expect(result!.points, hasLength(1));
      expect(result.points.first.waveHeightM, 1.4);
      expect(result.points.first.seaWaterTemperatureC, 11.3);
      expect(result.expiresAt, parseHttpDate('Sun, 17 May 2026 12:25:00 GMT'));
      expect(result.lastModified, 'Sun, 17 May 2026 11:55:00 GMT');
    });

    test('returns null on 404 (outside marine coverage)', () async {
      final client = MockClient((_) async => http.Response('not found', 404));
      final result =
          await YrOceanService(client: client).fetch(const LatLng(20, 30));
      expect(result, isNull);
    });

    test('returns null on 422 (unprocessable coord)', () async {
      final client = MockClient((_) async => http.Response('nope', 422));
      final result =
          await YrOceanService(client: client).fetch(const LatLng(20, 30));
      expect(result, isNull);
    });

    test('returns null when the response has an empty timeseries', () async {
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode({
            'properties': {'timeseries': []}
          }),
          200,
          headers: const {
            'content-type': 'application/json; charset=utf-8',
          },
        );
      });
      final result =
          await YrOceanService(client: client).fetch(const LatLng(20, 30));
      expect(result, isNull);
    });

    test('throws on 5xx (transient)', () async {
      final client = MockClient((_) async => http.Response('boom', 503));
      expect(
        () => YrOceanService(client: client).fetch(const LatLng(60, 5)),
        throwsA(isA<YrServiceException>()),
      );
    });

    test('on 304 reuses previous points and refreshes expiry', () async {
      final previous = MarineForecastResult(
        points: [
          MarinePoint(
            timeUtc: DateTime.utc(2026, 5, 17, 11),
            waveHeightM: 1.0,
            waveFromDeg: null,
            seaWaterTemperatureC: 10.0,
            seaWaterSpeedMs: null,
          ),
        ],
        expiresAt: DateTime.utc(2026, 5, 17, 11, 55),
        lastModified: 'Sun, 17 May 2026 10:55:00 GMT',
      );
      final client = MockClient((_) async {
        return http.Response('', 304, headers: const {
          'expires': 'Sun, 17 May 2026 12:55:00 GMT',
        });
      });
      final result = await YrOceanService(client: client).fetch(
        const LatLng(60, 5),
        ifModifiedSince: previous.lastModified,
        previous: previous,
      );
      expect(result, isNotNull);
      expect(result!.points, previous.points);
      expect(result.expiresAt, parseHttpDate('Sun, 17 May 2026 12:55:00 GMT'));
    });
  });
}
