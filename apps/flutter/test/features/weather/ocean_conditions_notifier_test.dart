import 'dart:convert';

import 'package:flutter_map/flutter_map.dart' show LatLngBounds;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/weather/api.dart';
import 'package:turbo/features/weather/data/weather_notifier.dart'
    show yrOceanServiceProvider;
import 'package:turbo/features/weather/data/yr_ocean_service.dart';

Map<String, dynamic> _payload(double waveHeight) => {
      'properties': {
        'timeseries': [
          {
            'time': '2026-05-17T12:00:00Z',
            'data': {
              'instant': {
                'details': {
                  'sea_surface_wave_height': waveHeight,
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

ProviderContainer _containerWith(YrOceanService service) {
  final c = ProviderContainer(overrides: [
    yrOceanServiceProvider.overrideWithValue(service),
  ]);
  addTearDown(c.dispose);
  return c;
}

// A small bounds (~0.5° square) off the Norwegian coast.
final _bounds = LatLngBounds(
  const LatLng(60.0, 5.0),
  const LatLng(60.5, 5.5),
);

void main() {
  group('OceanConditionsNotifier', () {
    test('samples a grid of sea points after the debounce window', () async {
      final client = MockClient((_) async {
        return http.Response(jsonEncode(_payload(1.4)), 200, headers: const {
          'content-type': 'application/json; charset=utf-8',
        });
      });
      final c = _containerWith(YrOceanService(client: client));

      c.read(oceanConditionsProvider.notifier).requestBounds(_bounds);
      // Wait past the 500ms debounce plus the concurrent fetch batch.
      await Future<void>.delayed(const Duration(milliseconds: 900));

      final samples = c.read(oceanConditionsProvider).value!;
      // 5×5 grid, all sea points.
      expect(samples, hasLength(25));
      expect(samples.every((s) => s.point.waveHeightM == 1.4), isTrue);
      // Every sampled coordinate falls inside the requested bounds.
      expect(
        samples.every((s) =>
            s.position.latitude >= 60.0 &&
            s.position.latitude <= 60.5 &&
            s.position.longitude >= 5.0 &&
            s.position.longitude <= 5.5),
        isTrue,
      );
    });

    test('excludes land / out-of-coverage points (404)', () async {
      final client = MockClient((_) async => http.Response('not found', 404));
      final c = _containerWith(YrOceanService(client: client));

      c.read(oceanConditionsProvider.notifier).requestBounds(_bounds);
      await Future<void>.delayed(const Duration(milliseconds: 900));

      expect(c.read(oceanConditionsProvider).value, isEmpty);
    });

    test('clear() empties the rendered samples', () async {
      final client = MockClient((_) async {
        return http.Response(jsonEncode(_payload(0.8)), 200, headers: const {
          'content-type': 'application/json; charset=utf-8',
        });
      });
      final c = _containerWith(YrOceanService(client: client));

      c.read(oceanConditionsProvider.notifier).requestBounds(_bounds);
      await Future<void>.delayed(const Duration(milliseconds: 900));
      expect(c.read(oceanConditionsProvider).value, isNotEmpty);

      c.read(oceanConditionsProvider.notifier).clear();
      expect(c.read(oceanConditionsProvider).value, isEmpty);
    });

    test('caches by coordinate so re-requesting the same bounds is cheap',
        () async {
      var requestCount = 0;
      final client = MockClient((_) async {
        requestCount++;
        return http.Response(jsonEncode(_payload(2.1)), 200, headers: const {
          'content-type': 'application/json; charset=utf-8',
        });
      });
      final c = _containerWith(YrOceanService(client: client));
      final notifier = c.read(oceanConditionsProvider.notifier);

      notifier.requestBounds(_bounds);
      await Future<void>.delayed(const Duration(milliseconds: 900));
      final firstPass = requestCount;
      expect(firstPass, 25);

      // Same bounds again — every cell is served from cache, no new requests.
      notifier.requestBounds(_bounds);
      await Future<void>.delayed(const Duration(milliseconds: 900));
      expect(requestCount, firstPass);
      expect(c.read(oceanConditionsProvider).value, hasLength(25));
    });
  });
}
