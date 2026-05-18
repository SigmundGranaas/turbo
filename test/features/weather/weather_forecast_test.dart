import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/weather/api.dart';

void main() {
  group('AtmosphericPoint.fromJson', () {
    test('parses the canonical compact/complete shape', () {
      final json = {
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
      };

      final p = AtmosphericPoint.fromJson(json);
      expect(p.timeUtc, DateTime.utc(2026, 5, 17, 12));
      expect(p.airTemperatureC, 14.2);
      expect(p.windSpeedMs, 3.1);
      expect(p.windFromDeg, 215.0);
      expect(p.humidity, 67.0);
      expect(p.pressureHpa, 1012.4);
      expect(p.cloudCoverPercent, 80.0);
      expect(p.uvIndex, 4.2);
      expect(p.precipitation1hMm, 0.4);
      expect(p.symbol1h?.code, 'rain');
      expect(p.symbol6h?.code, 'lightrainshowers_day');
      expect(p.symbol12h?.code, 'partlycloudy_day');
      expect(p.isSnowing, isFalse);
    });

    test('handles missing optional blocks gracefully', () {
      final json = {
        'time': '2026-05-17T15:00:00Z',
        'data': {
          'instant': {
            'details': {
              'air_temperature': 5.0,
              'wind_speed': 1.0,
            },
          },
        },
      };
      final p = AtmosphericPoint.fromJson(json);
      expect(p.airTemperatureC, 5.0);
      expect(p.windSpeedMs, 1.0);
      expect(p.precipitation1hMm, isNull);
      expect(p.symbol1h, isNull);
      expect(p.symbol6h, isNull);
      expect(p.uvIndex, isNull);
    });

    test('isSnowing reflects the 1h symbol when present', () {
      final p = AtmosphericPoint.fromJson({
        'time': '2026-01-10T09:00:00Z',
        'data': {
          'instant': {
            'details': {'air_temperature': -2.0, 'wind_speed': 4.0},
          },
          'next_1_hours': {
            'summary': {'symbol_code': 'heavysnowshowers_day'},
          },
        },
      });
      expect(p.isSnowing, isTrue);
    });
  });

  group('MarinePoint.fromJson', () {
    test('parses the ocean compact shape', () {
      final json = {
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
      };
      final p = MarinePoint.fromJson(json);
      expect(p.timeUtc, DateTime.utc(2026, 5, 17, 12));
      expect(p.waveHeightM, 1.4);
      expect(p.waveFromDeg, 280.0);
      expect(p.seaWaterTemperatureC, 11.3);
      expect(p.seaWaterSpeedMs, 0.2);
    });
  });

  group('WeatherForecast', () {
    final pos = const LatLng(60.39, 5.32);
    final now = DateTime.utc(2026, 5, 17, 12);
    AtmosphericPoint mkAtm(DateTime t, double temp,
            {WeatherSymbol? symbol1h}) =>
        AtmosphericPoint(
          timeUtc: t,
          airTemperatureC: temp,
          windSpeedMs: 0,
          windFromDeg: null,
          humidity: null,
          pressureHpa: null,
          cloudCoverPercent: null,
          uvIndex: null,
          precipitation1hMm: null,
          symbol1h: symbol1h,
          symbol6h: null,
          symbol12h: null,
        );

    test('isFresh reflects whether the fetched-source expiries are in the future',
        () {
      final fresh = WeatherForecast(
        position: pos,
        fetchedAt: now,
        atmosphericExpiresAt: now.add(const Duration(minutes: 30)),
        marineExpiresAt: now.add(const Duration(minutes: 30)),
        atmosphericLastModified: null,
        marineLastModified: null,
        atmospheric: [mkAtm(now, 10)],
        marine: const [],
      );
      // Pretend "now" is before expiry.
      expect(fresh.isFreshAt(now), isTrue);
      // After expiry: stale.
      expect(fresh.isFreshAt(now.add(const Duration(hours: 1))), isFalse);
    });

    test('hasMarineData is true only when the marine list is non-empty', () {
      final atmOnly = WeatherForecast(
        position: pos,
        fetchedAt: now,
        atmosphericExpiresAt: now.add(const Duration(minutes: 30)),
        marineExpiresAt: null,
        atmosphericLastModified: null,
        marineLastModified: null,
        atmospheric: [mkAtm(now, 10)],
        marine: const [],
      );
      expect(atmOnly.hasMarineData, isFalse);

      final withMarine = atmOnly.copyWith(
        marine: [
          MarinePoint(
            timeUtc: now,
            waveHeightM: 1.0,
            waveFromDeg: null,
            seaWaterTemperatureC: 10.0,
            seaWaterSpeedMs: null,
          ),
        ],
        marineExpiresAt: now.add(const Duration(minutes: 30)),
      );
      expect(withMarine.hasMarineData, isTrue);
    });

    test('dailySummaries groups atmospheric points by local date with min/max',
        () {
      // 4 points across 2 UTC days with varied temps + one snow symbol.
      final atm = [
        mkAtm(DateTime.utc(2026, 5, 17, 6), 5),
        mkAtm(DateTime.utc(2026, 5, 17, 12), 12,
            symbol1h: WeatherSymbol.fromCode('clearsky_day')),
        mkAtm(DateTime.utc(2026, 5, 17, 18), 9),
        mkAtm(DateTime.utc(2026, 5, 18, 12), 7,
            symbol1h: WeatherSymbol.fromCode('snow')),
      ];
      final f = WeatherForecast(
        position: pos,
        fetchedAt: now,
        atmosphericExpiresAt: now.add(const Duration(minutes: 30)),
        marineExpiresAt: null,
        atmosphericLastModified: null,
        marineLastModified: null,
        atmospheric: atm,
        marine: const [],
      );
      final summaries = f.dailySummaries();
      expect(summaries.length, 2);
      expect(summaries[0].minTempC, 5);
      expect(summaries[0].maxTempC, 12);
      expect(summaries[0].middaySymbol?.code, 'clearsky_day');
      expect(summaries[1].minTempC, 7);
      expect(summaries[1].maxTempC, 7);
      expect(summaries[1].middaySymbol?.code, 'snow');
    });
  });
}
