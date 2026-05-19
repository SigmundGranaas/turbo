import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/util/user_agent.dart';
import 'package:turbo/features/weather/api.dart';

Map<String, dynamic> _feature({
  required String level,
  required String event,
  required String onset,
  required String expires,
  String description = 'Heavy rain expected',
  List<List<List<double>>>? coords,
}) {
  return {
    'type': 'Feature',
    'properties': {
      'id': '$event-$level',
      'awareness_level': level,
      'event': event,
      'description': description,
      'onset': onset,
      'expires': expires,
    },
    'geometry': coords == null
        ? null
        : {
            'type': 'Polygon',
            'coordinates': [
              coords.first,
            ],
          },
  };
}

void main() {
  group('MetAlertsService.currentAtPoint', () {
    test('sends User-Agent + 4-decimal lat/lon to api.met.no', () async {
      http.Request? captured;
      final client = MockClient((req) async {
        captured = req;
        return http.Response(
          jsonEncode({'type': 'FeatureCollection', 'features': []}),
          200,
        );
      });
      final svc = MetAlertsService(client: client);
      await svc.currentAtPoint(const LatLng(60.39132, 5.32212));
      expect(captured!.url.host, 'api.met.no');
      expect(captured!.url.path, '/weatherapi/metalerts/2.0/current.json');
      expect(captured!.url.queryParameters['lat'], '60.3913');
      expect(captured!.url.queryParameters['lon'], '5.3221');
      expect(captured!.headers['User-Agent'], kTurboUserAgent);
    });

    test('parses awareness levels, event, time window, and polygon area',
        () async {
      final client = MockClient((_) async {
        return http.Response(
          jsonEncode({
            'type': 'FeatureCollection',
            'features': [
              _feature(
                level: '2; yellow; Moderate',
                event: 'rain',
                onset: '2026-05-17T12:00:00Z',
                expires: '2026-05-17T18:00:00Z',
                coords: [
                  [
                    [5.0, 60.0],
                    [5.1, 60.0],
                    [5.1, 60.1],
                    [5.0, 60.1],
                    [5.0, 60.0],
                  ],
                ],
              ),
            ],
          }),
          200,
        );
      });
      final result = await MetAlertsService(client: client)
          .currentAtPoint(const LatLng(60, 5));
      expect(result.alerts, hasLength(1));
      final alert = result.alerts.first;
      expect(alert.level, MetAlertLevel.yellow);
      expect(alert.event, 'rain');
      expect(alert.area, hasLength(5));
      expect(alert.area.first.latitude, 60.0);
      expect(alert.area.first.longitude, 5.0);
    });

    test('drops features that lack onset/expires or awareness level',
        () async {
      final client = MockClient((_) async => http.Response(
            jsonEncode({
              'type': 'FeatureCollection',
              'features': [
                {
                  'type': 'Feature',
                  'properties': {
                    'event': 'wind',
                    'awareness_level': 'orange',
                  },
                  'geometry': null,
                },
                _feature(
                  level: '3; orange',
                  event: 'wind',
                  onset: '2026-05-17T00:00:00Z',
                  expires: '2026-05-17T06:00:00Z',
                ),
              ],
            }),
            200,
          ));
      final result = await MetAlertsService(client: client)
          .currentAtPoint(const LatLng(60, 5));
      expect(result.alerts, hasLength(1));
      expect(result.alerts.first.level, MetAlertLevel.orange);
    });

    test('throws YrServiceException on 500', () async {
      final client =
          MockClient((_) async => http.Response('boom', 500));
      expect(
        () => MetAlertsService(client: client)
            .currentAtPoint(const LatLng(60, 5)),
        throwsA(isA<YrServiceException>()),
      );
    });
  });

  group('MetAlertsService.currentInBounds', () {
    test('fetches the global feed (no bbox param — MetAlerts rejects it '
        'with HTTP 400) and filters client-side by polygon bbox', () async {
      http.Request? captured;
      Map<String, Object> feature(String id, List<List<double>> ring) => {
            'type': 'Feature',
            'properties': {
              'id': id,
              'awareness_level': '2; yellow; Moderate',
              'event': 'Wind',
              'onset': '2026-05-19T00:00:00Z',
              'expires': '2026-05-19T23:00:00Z',
            },
            'geometry': {
              'type': 'Polygon',
              // GeoJSON uses [lon, lat]
              'coordinates': [ring],
            },
          };
      final client = MockClient((req) async {
        captured = req;
        return http.Response(
          jsonEncode({
            'type': 'FeatureCollection',
            'features': [
              // Southern Norway alert — should be kept.
              feature('south', [
                [5.0, 58.0],
                [12.0, 58.0],
                [12.0, 60.0],
                [5.0, 60.0],
                [5.0, 58.0],
              ]),
              // Far-northern alert — should be filtered out.
              feature('north', [
                [18.0, 69.0],
                [20.0, 69.0],
                [20.0, 71.0],
                [18.0, 71.0],
                [18.0, 69.0],
              ]),
            ],
          }),
          200,
          headers: {'content-type': 'application/json; charset=utf-8'},
        );
      });

      final result = await MetAlertsService(client: client)
          .currentInBounds(58.0, 5.0, 60.0, 12.0);

      // The API was called with no bbox query param (the previous
      // implementation sent bbox=… which the API rejects with HTTP 400).
      expect(captured!.url.queryParameters.containsKey('bbox'), isFalse,
          reason:
              'MetAlerts 2.0 current.json has no bbox parameter. Sending '
              'one returns 400.');
      // Client-side filter kept only the intersecting alert.
      expect(result.alerts.map((a) => a.id), contains('south'));
      expect(result.alerts.map((a) => a.id), isNot(contains('north')));
    });
  });
}
