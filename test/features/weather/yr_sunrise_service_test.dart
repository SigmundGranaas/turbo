import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/weather/api.dart';

Map<String, dynamic> _sunPayload({
  String sunrise = '2026-05-17T04:31:00+02:00',
  String sunset = '2026-05-17T22:14:00+02:00',
  String solarNoon = '2026-05-17T13:22:00+02:00',
  bool visibleNoon = true,
  bool includeSunrise = true,
  bool includeSunset = true,
}) {
  return {
    'properties': {
      if (includeSunrise) 'sunrise': {'time': sunrise},
      if (includeSunset) 'sunset': {'time': sunset},
      'solarnoon': {'time': solarNoon, 'visible': visibleNoon},
      'solarmidnight': {'time': '2026-05-17T01:22:00+02:00', 'visible': false},
    },
  };
}

Map<String, dynamic> _moonPayload({double phase = 90}) => {
      'properties': {
        'moonrise': {'time': '2026-05-17T06:11:00+02:00'},
        'moonset': {'time': '2026-05-17T19:22:00+02:00'},
        'moonphase': phase,
      },
    };

void main() {
  group('YrSunriseService.fetch', () {
    test('issues one sun + one moon request per day, rounded to 4 decimals',
        () async {
      final calls = <Uri>[];
      final client = MockClient((req) async {
        calls.add(req.url);
        if (req.url.path.endsWith('/sun')) {
          return http.Response(jsonEncode(_sunPayload()), 200);
        }
        return http.Response(jsonEncode(_moonPayload()), 200);
      });
      final svc = YrSunriseService(client: client);
      final now = DateTime(2026, 5, 17, 12);
      await svc.fetch(const LatLng(59.913889, 10.752222), days: 2, now: now);

      expect(calls.length, 4); // sun+moon for 2 days
      final sunCall =
          calls.firstWhere((u) => u.path.endsWith('/sun'));
      expect(sunCall.host, 'api.met.no');
      expect(sunCall.queryParameters['lat'], '59.9139');
      expect(sunCall.queryParameters['lon'], '10.7522');
      expect(sunCall.queryParameters['date'], '2026-05-17');
      expect(sunCall.queryParameters['offset'], isNotEmpty);
    });

    test('attaches User-Agent on every request', () async {
      final headersSeen = <String>{};
      final client = MockClient((req) async {
        headersSeen.add(req.headers['User-Agent'] ?? '');
        return http.Response(jsonEncode(_sunPayload()), 200);
      });
      final svc = YrSunriseService(client: client);
      await svc.fetch(const LatLng(60, 5),
          days: 1, now: DateTime(2026, 5, 17));
      expect(headersSeen, contains(kTurboUserAgent));
    });

    test('parses sunrise / sunset / solar noon into SunEvent', () async {
      final client = MockClient((req) async {
        if (req.url.path.endsWith('/sun')) {
          return http.Response(jsonEncode(_sunPayload()), 200);
        }
        return http.Response(jsonEncode(_moonPayload(phase: 180)), 200);
      });
      final svc = YrSunriseService(client: client);
      final result = await svc.fetch(const LatLng(60, 5),
          days: 1, now: DateTime(2026, 5, 17));
      expect(result.sun, hasLength(1));
      final event = result.sun.values.first;
      expect(event.sunrise!.toUtc(),
          DateTime.parse('2026-05-17T04:31:00+02:00').toUtc());
      expect(event.sunset!.toUtc(),
          DateTime.parse('2026-05-17T22:14:00+02:00').toUtc());
      expect(event.daylight!.inMinutes, greaterThan(60 * 17));
      expect(event.polarDay, isFalse);
      expect(event.polarNight, isFalse);
      final moon = result.moon.values.first;
      expect(moon.phaseDegrees, 180);
      expect(moon.illumination, closeTo(1.0, 1e-6));
    });

    test('detects polar day when sunrise/sunset omitted and noon visible',
        () async {
      final client = MockClient((req) async {
        if (req.url.path.endsWith('/sun')) {
          return http.Response(
              jsonEncode(_sunPayload(
                includeSunrise: false,
                includeSunset: false,
                visibleNoon: true,
              )),
              200);
        }
        return http.Response(jsonEncode(_moonPayload()), 200);
      });
      final svc = YrSunriseService(client: client);
      final result = await svc.fetch(const LatLng(78, 16),
          days: 1, now: DateTime(2026, 6, 21));
      final event = result.sun.values.first;
      expect(event.polarDay, isTrue);
      expect(event.polarNight, isFalse);
      expect(event.daylight, const Duration(hours: 24));
    });

    test('detects polar night when sunrise/sunset omitted and noon invisible',
        () async {
      final client = MockClient((req) async {
        if (req.url.path.endsWith('/sun')) {
          return http.Response(
              jsonEncode(_sunPayload(
                includeSunrise: false,
                includeSunset: false,
                visibleNoon: false,
              )),
              200);
        }
        return http.Response(jsonEncode(_moonPayload()), 200);
      });
      final svc = YrSunriseService(client: client);
      final result = await svc.fetch(const LatLng(78, 16),
          days: 1, now: DateTime(2026, 12, 21));
      final event = result.sun.values.first;
      expect(event.polarNight, isTrue);
      expect(event.daylight, Duration.zero);
    });
  });
}
