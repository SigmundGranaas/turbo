import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/weather/api.dart';

class _RecordingAtm implements YrAtmosphericService {
  int calls = 0;
  AtmosphericForecastResult? next;
  Object? error;
  String? lastIfModifiedSince;
  AtmosphericForecastResult? lastPrevious;

  @override
  Future<AtmosphericForecastResult> fetch(
    LatLng position, {
    String? ifModifiedSince,
    AtmosphericForecastResult? previous,
  }) async {
    calls++;
    lastIfModifiedSince = ifModifiedSince;
    lastPrevious = previous;
    if (error != null) throw error!;
    return next!;
  }
}

class _RecordingOcean implements YrOceanService {
  int calls = 0;
  MarineForecastResult? next;
  Object? error;
  String? lastIfModifiedSince;
  MarineForecastResult? lastPrevious;

  @override
  Future<MarineForecastResult?> fetch(
    LatLng position, {
    String? ifModifiedSince,
    MarineForecastResult? previous,
  }) async {
    calls++;
    lastIfModifiedSince = ifModifiedSince;
    lastPrevious = previous;
    if (error != null) throw error!;
    return next;
  }
}

class _StubSunrise implements YrSunriseService {
  @override
  Future<SunriseForecastResult> fetch(
    LatLng position, {
    int days = 9,
    DateTime? now,
    String? ifModifiedSince,
    SunriseForecastResult? previous,
  }) async {
    return SunriseForecastResult(
      sun: const {},
      moon: const {},
      expiresAt: DateTime.utc(2026, 5, 17, 12, 30),
      lastModified: null,
    );
  }
}

class _StubAlerts implements MetAlertsService {
  @override
  Future<MetAlertsResult> currentAtPoint(LatLng position,
      {String? ifModifiedSince}) async {
    return MetAlertsResult(
      alerts: const [],
      expiresAt: DateTime.utc(2026, 5, 17, 12, 30),
      lastModified: null,
    );
  }
}

WeatherFetcher _fetcher(
        _RecordingAtm atm, _RecordingOcean ocean) =>
    WeatherFetcher(
      atmospheric: atm,
      ocean: ocean,
      sunrise: _StubSunrise(),
      alerts: _StubAlerts(),
    );

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
      lastModified: 'atm-tag',
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
      lastModified: 'marine-tag',
    );

void main() {
  group('WeatherFetcher.fetch', () {
    test('always hits both services in parallel', () async {
      final atm = _RecordingAtm()..next = _atmFixture();
      final ocean = _RecordingOcean()..next = _marineFixture();
      final fetcher = _fetcher(atm, ocean);

      final f = await fetcher.fetch(const LatLng(60, 5));

      expect(atm.calls, 1);
      expect(ocean.calls, 1);
      expect(f.atmospheric, hasLength(1));
      expect(f.marine, hasLength(1));
      expect(f.hasMarineData, isTrue);
    });

    test('marine returning null leaves marine empty, atmospheric intact',
        () async {
      final atm = _RecordingAtm()..next = _atmFixture();
      final ocean = _RecordingOcean()..next = null;
      final fetcher = _fetcher(atm, ocean);

      final f = await fetcher.fetch(const LatLng(60, 5));

      expect(f.atmospheric, hasLength(1));
      expect(f.marine, isEmpty);
      expect(f.hasMarineData, isFalse);
      expect(f.marineExpiresAt, isNull);
    });

    test('marine throwing is swallowed; atmospheric data still returned',
        () async {
      final atm = _RecordingAtm()..next = _atmFixture();
      final ocean = _RecordingOcean()..error = Exception('marine boom');
      final fetcher = _fetcher(atm, ocean);

      final f = await fetcher.fetch(const LatLng(60, 5));

      expect(f.atmospheric, hasLength(1));
      expect(f.marine, isEmpty);
    });

    test('atmospheric failure surfaces as a thrown exception', () async {
      final atm = _RecordingAtm()..error = const YrServiceException(503, 'x');
      final ocean = _RecordingOcean()..next = _marineFixture();
      final fetcher = _fetcher(atm, ocean);

      expect(
        () => fetcher.fetch(const LatLng(60, 5)),
        throwsA(isA<YrServiceException>()),
      );
    });

    test('forwards previous Last-Modified for conditional revalidation',
        () async {
      final atm = _RecordingAtm()..next = _atmFixture();
      final ocean = _RecordingOcean()..next = _marineFixture();
      final fetcher = _fetcher(atm, ocean);

      final previous = WeatherForecast(
        position: const LatLng(60, 5),
        fetchedAt: DateTime.utc(2026, 5, 17, 11),
        atmosphericExpiresAt: DateTime.utc(2026, 5, 17, 11, 30),
        marineExpiresAt: DateTime.utc(2026, 5, 17, 11, 30),
        atmosphericLastModified: 'atm-prev',
        marineLastModified: 'marine-prev',
        atmospheric: atm.next!.points,
        marine: ocean.next!.points,
      );
      await fetcher.fetch(const LatLng(60, 5), previous: previous);

      expect(atm.lastIfModifiedSince, 'atm-prev');
      expect(ocean.lastIfModifiedSince, 'marine-prev');
      expect(atm.lastPrevious, isNotNull);
      expect(ocean.lastPrevious, isNotNull);
    });
  });
}
