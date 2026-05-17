import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:http_parser/http_parser.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/weather/api.dart';

const _samplePayload = {
  'properties': {
    'timeseries': [
      {
        'time': '2026-05-17T12:00:00Z',
        'data': {
          'instant': {
            'details': {
              'air_temperature': 14.2,
              'wind_speed': 3.1,
              'wind_from_direction': 215.0,
              'relative_humidity': 67.0,
              'air_pressure_at_sea_level': 1012.4,
              'cloud_area_fraction': 80.0,
              'ultraviolet_index_clear_sky': 4.2,
            },
          },
          'next_1_hours': {
            'summary': {'symbol_code': 'rain'},
            'details': {'precipitation_amount': 0.4},
          },
          'next_6_hours': {
            'summary': {'symbol_code': 'lightrainshowers_day'},
          },
          'next_12_hours': {
            'summary': {'symbol_code': 'partlycloudy_day'},
          },
        },
      },
      {
        'time': '2026-05-17T13:00:00Z',
        'data': {
          'instant': {
            'details': {'air_temperature': 15.0, 'wind_speed': 3.4},
          },
        },
      },
    ],
  },
};

void main() {
  group('YrAtmosphericService.fetch', () {
    test('encodes lat/lon to 4 decimals and sends required headers', () async {
      http.Request? captured;
      final client = MockClient((req) async {
        captured = req;
        return http.Response(
          jsonEncode(_samplePayload),
          200,
          headers: {
            'content-type': 'application/json; charset=utf-8',
            'last-modified': 'Sun, 17 May 2026 11:55:00 GMT',
            'expires': 'Sun, 17 May 2026 12:25:00 GMT',
          },
        );
      });

      final svc = YrAtmosphericService(client: client);
      await svc.fetch(const LatLng(59.913889, 10.752222));

      expect(captured, isNotNull);
      expect(captured!.url.host, 'api.met.no');
      expect(captured!.url.path,
          '/weatherapi/locationforecast/2.0/complete');
      expect(captured!.url.queryParameters['lat'], '59.9139');
      expect(captured!.url.queryParameters['lon'], '10.7522');
      expect(captured!.headers['User-Agent'], kTurboUserAgent);
      expect(captured!.headers['Accept'], 'application/json');
      expect(captured!.headers.containsKey('If-Modified-Since'), isFalse);
    });

    test('parses 200 response into AtmosphericForecastResult', () async {
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode(_samplePayload),
          200,
          headers: {
            'content-type': 'application/json; charset=utf-8',
            'last-modified': 'Sun, 17 May 2026 11:55:00 GMT',
            'expires': 'Sun, 17 May 2026 12:25:00 GMT',
          },
        );
      });
      final result =
          await YrAtmosphericService(client: client).fetch(const LatLng(60, 5));

      expect(result.points, hasLength(2));
      expect(result.points.first.airTemperatureC, 14.2);
      expect(result.points.first.symbol1h?.code, 'rain');
      expect(result.lastModified, 'Sun, 17 May 2026 11:55:00 GMT');
      expect(result.expiresAt, parseHttpDate('Sun, 17 May 2026 12:25:00 GMT'));
    });

    test('defaults expiresAt to fetchedAt + 30m when header is missing',
        () async {
      final before = DateTime.now().toUtc();
      final client = MockClient((_) async {
        return http.Response(jsonEncode(_samplePayload), 200, headers: const {
          'content-type': 'application/json; charset=utf-8',
        });
      });
      final result =
          await YrAtmosphericService(client: client).fetch(const LatLng(60, 5));
      final after = DateTime.now().toUtc();
      final lower = before.add(const Duration(minutes: 29, seconds: 50));
      final upper = after.add(const Duration(minutes: 30, seconds: 10));
      expect(result.expiresAt.isAfter(lower), isTrue);
      expect(result.expiresAt.isBefore(upper), isTrue);
      expect(result.lastModified, isNull);
    });

    test('sends If-Modified-Since when previous lastModified is provided',
        () async {
      String? sentIfMod;
      final client = MockClient((req) async {
        sentIfMod = req.headers['If-Modified-Since'];
        return http.Response(jsonEncode(_samplePayload), 200, headers: const {
          'content-type': 'application/json; charset=utf-8',
          'expires': 'Sun, 17 May 2026 12:25:00 GMT',
        });
      });
      await YrAtmosphericService(client: client).fetch(
        const LatLng(60, 5),
        ifModifiedSince: 'Sun, 17 May 2026 11:55:00 GMT',
      );
      expect(sentIfMod, 'Sun, 17 May 2026 11:55:00 GMT');
    });

    test('on 304 reuses previous points and refreshes expiry', () async {
      final previous = AtmosphericForecastResult(
        points: [
          AtmosphericPoint(
            timeUtc: DateTime.utc(2026, 5, 17, 11),
            airTemperatureC: 10,
            windSpeedMs: 1,
            windFromDeg: null,
            humidity: null,
            pressureHpa: null,
            cloudCoverPercent: null,
            uvIndex: null,
            precipitation1hMm: null,
            symbol1h: null,
            symbol6h: null,
            symbol12h: null,
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
      final result = await YrAtmosphericService(client: client).fetch(
        const LatLng(60, 5),
        ifModifiedSince: previous.lastModified,
        previous: previous,
      );
      expect(result.points, previous.points);
      expect(result.expiresAt, parseHttpDate('Sun, 17 May 2026 12:55:00 GMT'));
      // last-modified carries over
      expect(result.lastModified, previous.lastModified);
    });

    test('throws YrServiceException on 5xx', () async {
      final client = MockClient((_) async => http.Response('boom', 503));
      expect(
        () => YrAtmosphericService(client: client).fetch(const LatLng(60, 5)),
        throwsA(isA<YrServiceException>()),
      );
    });

    test('throws YrServiceException on 4xx', () async {
      final client = MockClient((_) async => http.Response('bad', 403));
      expect(
        () => YrAtmosphericService(client: client).fetch(const LatLng(60, 5)),
        throwsA(isA<YrServiceException>()),
      );
    });

    test('still parses but does not crash on 203 (deprecated version)',
        () async {
      final client = MockClient((_) async {
        return http.Response(jsonEncode(_samplePayload), 203, headers: const {
          'content-type': 'application/json; charset=utf-8',
          'expires': 'Sun, 17 May 2026 12:25:00 GMT',
        });
      });
      final result =
          await YrAtmosphericService(client: client).fetch(const LatLng(60, 5));
      expect(result.points, hasLength(2));
    });
  });
}
