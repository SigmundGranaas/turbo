import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http_mock_adapter/http_mock_adapter.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/routing/api.dart';

void main() {
  const base = 'http://test.local/v1/route';
  late Dio dio;
  late DioAdapter adapter;
  late RoutingApiClient client;

  setUp(() {
    dio = Dio(BaseOptions());
    adapter = DioAdapter(dio: dio);
    client = RoutingApiClient(baseUrl: base, dio: dio);
  });

  group('plan', () {
    test('parses a solved route, converting [lon,lat] → LatLng', () async {
      adapter.onPost(
        '$base/plan',
        (server) => server.reply(200, {
          'distance_m': 1234.5,
          'duration_s': 1800.0,
          'ascent_m': 210.0,
          'on_trail_pct': 62.5,
          'surfaces': {'trail': 800.0, 'off_trail': 434.5},
          'geometry': {
            'type': 'LineString',
            'coordinates': [
              [15.371, 67.398],
              [15.380, 67.404],
            ],
          },
          'legs': [
            {'from_index': 0, 'to_index': 1, 'distance_m': 1234.5},
          ],
        }),
        data: Matchers.any,
      );

      final plan = await client.plan(RouteRequest(
        points: const [LatLng(67.398, 15.371), LatLng(67.404, 15.380)],
        preset: 'balanced',
      ));

      expect(plan.distanceM, 1234.5);
      expect(plan.duration, const Duration(minutes: 30));
      expect(plan.ascentM, 210.0);
      expect(plan.onTrailPct, 62.5);
      expect(plan.surfaces['trail'], 800.0);
      expect(plan.geometry, hasLength(2));
      // GeoJSON [lon, lat] must map to LatLng(lat, lon).
      expect(plan.geometry.first.latitude, closeTo(67.398, 1e-9));
      expect(plan.geometry.first.longitude, closeTo(15.371, 1e-9));
      expect(plan.legs.single.toIndex, 1);
    });

    test('serializes points as [lon, lat] pairs', () {
      final json = RouteRequest(
        points: const [LatLng(67.398, 15.371), LatLng(67.404, 15.380)],
        profile: 'foot',
      ).toJson();

      expect(json['points'], [
        [15.371, 67.398],
        [15.380, 67.404],
      ]);
      expect(json['profile'], 'foot');
      expect(json.containsKey('preset'), isFalse);
    });

    test('400 → RoutingException(badRequest) with envelope message', () async {
      adapter.onPost(
        '$base/plan',
        (server) => server.reply(400, {'error': 'need at least 2 points'}),
        data: Matchers.any,
      );

      await expectLater(
        client.plan(const RouteRequest(points: [LatLng(0, 0)])),
        throwsA(isA<RoutingException>()
            .having((e) => e.kind, 'kind', RoutingErrorKind.badRequest)
            .having((e) => e.message, 'message', 'need at least 2 points')),
      );
    });

    test('422 → RoutingException(noRoute) carrying details', () async {
      adapter.onPost(
        '$base/plan',
        (server) => server.reply(422, {
          'error': 'no route',
          'details': {'kind': 'segment_failed', 'leg_index': 1},
        }),
        data: Matchers.any,
      );

      await expectLater(
        client.plan(const RouteRequest(
          points: [LatLng(67.0, 15.0), LatLng(67.1, 15.1)],
        )),
        throwsA(isA<RoutingException>()
            .having((e) => e.kind, 'kind', RoutingErrorKind.noRoute)
            .having((e) => e.details?['leg_index'], 'details.leg_index', 1)),
      );
    });

    test('5xx → RoutingException(server)', () async {
      adapter.onPost(
        '$base/plan',
        (server) => server.reply(500, {'error': 'boom'}),
        data: Matchers.any,
      );

      await expectLater(
        client.plan(const RouteRequest(
          points: [LatLng(67.0, 15.0), LatLng(67.1, 15.1)],
        )),
        throwsA(isA<RoutingException>()
            .having((e) => e.kind, 'kind', RoutingErrorKind.server)),
      );
    });
  });

  group('presets', () {
    test('parses the preset list', () async {
      adapter.onGet(
        '$base/presets',
        (server) => server.reply(200, [
          {'name': 'balanced', 'label': 'Balanced', 'description': 'default'},
          {'name': 'direct', 'label': 'Direct', 'description': 'shortest'},
        ]),
      );

      final presets = await client.presets();

      expect(presets, hasLength(2));
      expect(presets.first.name, 'balanced');
      expect(presets.last.label, 'Direct');
    });
  });
}
