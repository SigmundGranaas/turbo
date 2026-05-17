import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/weather/api.dart';
import 'package:turbo/features/weather/data/weather_fetcher.dart';

class _RecordingAtm implements YrAtmosphericService {
  int calls = 0;
  AtmosphericForecastResult? next;
  Object? error;

  @override
  Future<AtmosphericForecastResult> fetch(
    LatLng position, {
    String? ifModifiedSince,
    AtmosphericForecastResult? previous,
  }) async {
    calls++;
    if (error != null) throw error!;
    return next!;
  }
}

class _RecordingOcean implements YrOceanService {
  int calls = 0;
  MarineForecastResult? next;
  Object? error;

  @override
  Future<MarineForecastResult?> fetch(
    LatLng position, {
    String? ifModifiedSince,
    MarineForecastResult? previous,
  }) async {
    calls++;
    if (error != null) throw error!;
    return next;
  }
}

AtmosphericForecastResult _atmFixture() => AtmosphericForecastResult(
      points: [
        AtmosphericPoint(
          timeUtc: DateTime.utc(2026, 5, 17, 12),
          airTemperatureC: 14.2,
          windSpeedMs: 3.1,
          windFromDeg: null,
          humidity: null,
          pressureHpa: null,
          cloudCoverPercent: null,
          uvIndex: null,
          precipitation1hMm: null,
          symbol1h: null,
          symbol6h: null,
          symbol12h: null,
        )
      ],
      expiresAt: DateTime.utc(2026, 5, 17, 12, 30),
      lastModified: 'Sun, 17 May 2026 11:55:00 GMT',
    );

MarineForecastResult _marineFixture() => MarineForecastResult(
      points: [
        MarinePoint(
          timeUtc: DateTime.utc(2026, 5, 17, 12),
          waveHeightM: 1.2,
          waveFromDeg: 280,
          seaWaterTemperatureC: 11.3,
          seaWaterSpeedMs: 0.2,
        )
      ],
      expiresAt: DateTime.utc(2026, 5, 17, 12, 30),
      lastModified: 'Sun, 17 May 2026 11:55:00 GMT',
    );

void main() {
  group('WeatherFetcher.fetch', () {
    test('atmospheric-only request hits only the atmospheric service',
        () async {
      final atm = _RecordingAtm()..next = _atmFixture();
      final ocean = _RecordingOcean();
      final fetcher = WeatherFetcher(atmospheric: atm, ocean: ocean);

      final forecast = await fetcher.fetch(
        const LatLng(60, 5),
        const {WeatherMetricSource.atmospheric},
      );

      expect(atm.calls, 1);
      expect(ocean.calls, 0);
      expect(forecast.atmospheric, hasLength(1));
      expect(forecast.marine, isEmpty);
      expect(forecast.atmosphericExpiresAt, atm.next!.expiresAt);
      expect(forecast.marineExpiresAt, isNull);
    });

    test('marine-only request hits only the ocean service', () async {
      final atm = _RecordingAtm();
      final ocean = _RecordingOcean()..next = _marineFixture();
      final fetcher = WeatherFetcher(atmospheric: atm, ocean: ocean);

      final forecast = await fetcher.fetch(
        const LatLng(60, 5),
        const {WeatherMetricSource.marine},
      );

      expect(atm.calls, 0);
      expect(ocean.calls, 1);
      expect(forecast.marine, hasLength(1));
      expect(forecast.atmospheric, isEmpty);
      expect(forecast.marineExpiresAt, ocean.next!.expiresAt);
    });

    test('both-source request hits both services in parallel', () async {
      final atm = _RecordingAtm()..next = _atmFixture();
      final ocean = _RecordingOcean()..next = _marineFixture();
      final fetcher = WeatherFetcher(atmospheric: atm, ocean: ocean);

      final forecast = await fetcher.fetch(
        const LatLng(60, 5),
        const {WeatherMetricSource.atmospheric, WeatherMetricSource.marine},
      );

      expect(atm.calls, 1);
      expect(ocean.calls, 1);
      expect(forecast.atmospheric, hasLength(1));
      expect(forecast.marine, hasLength(1));
      expect(forecast.hasMarineData, isTrue);
    });

    test('marine failure does not corrupt the atmospheric side', () async {
      final atm = _RecordingAtm()..next = _atmFixture();
      final ocean = _RecordingOcean()..error = Exception('marine boom');
      final fetcher = WeatherFetcher(atmospheric: atm, ocean: ocean);

      final forecast = await fetcher.fetch(
        const LatLng(60, 5),
        const {WeatherMetricSource.atmospheric, WeatherMetricSource.marine},
      );

      expect(forecast.atmospheric, hasLength(1));
      expect(forecast.marine, isEmpty);
      expect(forecast.marineExpiresAt, isNull);
    });

    test('atmospheric failure surfaces as a thrown exception', () async {
      final atm = _RecordingAtm()..error = const YrServiceException(503, 'x');
      final ocean = _RecordingOcean()..next = _marineFixture();
      final fetcher = WeatherFetcher(atmospheric: atm, ocean: ocean);

      expect(
        () => fetcher.fetch(
          const LatLng(60, 5),
          const {WeatherMetricSource.atmospheric, WeatherMetricSource.marine},
        ),
        throwsA(isA<YrServiceException>()),
      );
    });

    test('marine endpoint returning null leaves marine empty', () async {
      final atm = _RecordingAtm()..next = _atmFixture();
      final ocean = _RecordingOcean()..next = null;
      final fetcher = WeatherFetcher(atmospheric: atm, ocean: ocean);

      final forecast = await fetcher.fetch(
        const LatLng(60, 5),
        const {WeatherMetricSource.atmospheric, WeatherMetricSource.marine},
      );

      expect(forecast.hasMarineData, isFalse);
      expect(forecast.marineExpiresAt, isNull);
    });

    test('forwards previous result so conditional requests can revalidate',
        () async {
      String? atmIfMod;
      String? marineIfMod;
      AtmosphericForecastResult? atmPrev;
      MarineForecastResult? marinePrev;

      final atm = _RecordingAtm()..next = _atmFixture();
      final ocean = _RecordingOcean()..next = _marineFixture();

      // Replace the simple fakes with capturing ones for this test.
      final capturingAtm = _CapturingAtm()..next = _atmFixture();
      final capturingOcean = _CapturingOcean()..next = _marineFixture();
      final fetcher =
          WeatherFetcher(atmospheric: capturingAtm, ocean: capturingOcean);

      final previous = WeatherForecast(
        position: const LatLng(60, 5),
        fetchedAt: DateTime.utc(2026, 5, 17, 11),
        atmosphericExpiresAt: DateTime.utc(2026, 5, 17, 11, 30),
        marineExpiresAt: DateTime.utc(2026, 5, 17, 11, 30),
        atmosphericLastModified: 'atm-tag',
        marineLastModified: 'marine-tag',
        atmospheric: atm.next!.points,
        marine: ocean.next!.points,
      );
      await fetcher.fetch(
        const LatLng(60, 5),
        const {WeatherMetricSource.atmospheric, WeatherMetricSource.marine},
        previous: previous,
      );
      atmIfMod = capturingAtm.lastIfModifiedSince;
      marineIfMod = capturingOcean.lastIfModifiedSince;
      atmPrev = capturingAtm.lastPrevious;
      marinePrev = capturingOcean.lastPrevious;

      expect(atmIfMod, 'atm-tag');
      expect(marineIfMod, 'marine-tag');
      expect(atmPrev, isNotNull);
      expect(marinePrev, isNotNull);
    });
  });
}

class _CapturingAtm implements YrAtmosphericService {
  String? lastIfModifiedSince;
  AtmosphericForecastResult? lastPrevious;
  AtmosphericForecastResult? next;

  @override
  Future<AtmosphericForecastResult> fetch(
    LatLng position, {
    String? ifModifiedSince,
    AtmosphericForecastResult? previous,
  }) async {
    lastIfModifiedSince = ifModifiedSince;
    lastPrevious = previous;
    return next!;
  }
}

class _CapturingOcean implements YrOceanService {
  String? lastIfModifiedSince;
  MarineForecastResult? lastPrevious;
  MarineForecastResult? next;

  @override
  Future<MarineForecastResult?> fetch(
    LatLng position, {
    String? ifModifiedSince,
    MarineForecastResult? previous,
  }) async {
    lastIfModifiedSince = ifModifiedSince;
    lastPrevious = previous;
    return next;
  }
}
