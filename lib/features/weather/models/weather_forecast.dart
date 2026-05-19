import 'package:latlong2/latlong.dart';

import 'met_alert.dart';
import 'sun_event.dart';
import 'weather_symbol.dart';

/// One point in MET's atmospheric `locationforecast/2.0` timeseries.
class AtmosphericPoint {
  final DateTime timeUtc;
  final double airTemperatureC;
  final double windSpeedMs;

  /// Peak wind gust in m/s when MET reports one. Optional in the
  /// locationforecast payload — small-boat users care about gusts more than
  /// sustained wind, so we surface it explicitly.
  final double? windGustMs;
  final double? windFromDeg;
  final double? humidity;
  final double? pressureHpa;
  final double? cloudCoverPercent;
  final double? uvIndex;
  final double? precipitation1hMm;
  final WeatherSymbol? symbol1h;
  final WeatherSymbol? symbol6h;
  final WeatherSymbol? symbol12h;

  const AtmosphericPoint({
    required this.timeUtc,
    required this.airTemperatureC,
    required this.windSpeedMs,
    this.windGustMs,
    required this.windFromDeg,
    required this.humidity,
    required this.pressureHpa,
    required this.cloudCoverPercent,
    required this.uvIndex,
    required this.precipitation1hMm,
    required this.symbol1h,
    required this.symbol6h,
    required this.symbol12h,
  });

  bool get isSnowing => symbol1h?.isSnow ?? false;

  static double? _asDouble(Object? v) =>
      v == null ? null : (v as num).toDouble();

  static WeatherSymbol? _symbolFromBlock(Map<String, dynamic>? block) {
    final summary = block?['summary'] as Map<String, dynamic>?;
    final code = summary?['symbol_code'] as String?;
    if (code == null) return null;
    return WeatherSymbol.fromCode(code);
  }

  factory AtmosphericPoint.fromJson(Map<String, dynamic> json) {
    final data = json['data'] as Map<String, dynamic>;
    final instant = (data['instant'] as Map<String, dynamic>?)?['details']
            as Map<String, dynamic>? ??
        const {};
    final next1h = data['next_1_hours'] as Map<String, dynamic>?;
    final next6h = data['next_6_hours'] as Map<String, dynamic>?;
    final next12h = data['next_12_hours'] as Map<String, dynamic>?;
    final next1hDetails =
        next1h?['details'] as Map<String, dynamic>? ?? const {};

    return AtmosphericPoint(
      timeUtc: DateTime.parse(json['time'] as String).toUtc(),
      airTemperatureC: (instant['air_temperature'] as num).toDouble(),
      windSpeedMs: (instant['wind_speed'] as num).toDouble(),
      windGustMs: _asDouble(instant['wind_speed_of_gust']),
      windFromDeg: _asDouble(instant['wind_from_direction']),
      humidity: _asDouble(instant['relative_humidity']),
      pressureHpa: _asDouble(instant['air_pressure_at_sea_level']),
      cloudCoverPercent: _asDouble(instant['cloud_area_fraction']),
      uvIndex: _asDouble(instant['ultraviolet_index_clear_sky']),
      precipitation1hMm: _asDouble(next1hDetails['precipitation_amount']),
      symbol1h: _symbolFromBlock(next1h),
      symbol6h: _symbolFromBlock(next6h),
      symbol12h: _symbolFromBlock(next12h),
    );
  }
}

/// One point in MET's `oceanforecast/2.0` marine timeseries.
class MarinePoint {
  final DateTime timeUtc;
  final double? waveHeightM;
  final double? waveFromDeg;
  final double? seaWaterTemperatureC;
  final double? seaWaterSpeedMs;

  const MarinePoint({
    required this.timeUtc,
    required this.waveHeightM,
    required this.waveFromDeg,
    required this.seaWaterTemperatureC,
    required this.seaWaterSpeedMs,
  });

  static double? _asDouble(Object? v) =>
      v == null ? null : (v as num).toDouble();

  factory MarinePoint.fromJson(Map<String, dynamic> json) {
    final details = ((json['data'] as Map<String, dynamic>?)?['instant']
            as Map<String, dynamic>?)?['details'] as Map<String, dynamic>? ??
        const {};
    return MarinePoint(
      timeUtc: DateTime.parse(json['time'] as String).toUtc(),
      waveHeightM: _asDouble(details['sea_surface_wave_height']),
      waveFromDeg: _asDouble(details['sea_surface_wave_from_direction']),
      seaWaterTemperatureC: _asDouble(details['sea_water_temperature']),
      seaWaterSpeedMs: _asDouble(details['sea_water_speed']),
    );
  }
}

/// Per-day rollup derived from a `WeatherForecast`'s atmospheric timeseries.
class DailySummary {
  final DateTime date;
  final double minTempC;
  final double maxTempC;
  final WeatherSymbol? middaySymbol;
  final double precipitationTotalMm;

  const DailySummary({
    required this.date,
    required this.minTempC,
    required this.maxTempC,
    required this.middaySymbol,
    required this.precipitationTotalMm,
  });
}

/// A merged forecast for a coordinate. Either or both timeseries lists may be
/// empty (atmospheric is normally populated; marine is empty outside MET's
/// Nordic-seas coverage).
class WeatherForecast {
  final LatLng position;
  final DateTime fetchedAt;
  final DateTime atmosphericExpiresAt;
  final DateTime? marineExpiresAt;
  final String? atmosphericLastModified;
  final String? marineLastModified;
  final List<AtmosphericPoint> atmospheric;
  final List<MarinePoint> marine;

  /// Sun events keyed by local date (midnight). Empty when the Sunrise 3
  /// fetch failed or is still pending — the UI hides the sun strip in that
  /// case.
  final Map<DateTime, SunEvent> sun;

  /// Moon events keyed by local date. Optional, may be empty.
  final Map<DateTime, MoonEvent> moon;

  /// Active MetAlerts that intersect [position]. Empty when none.
  final List<MetAlert> alerts;

  const WeatherForecast({
    required this.position,
    required this.fetchedAt,
    required this.atmosphericExpiresAt,
    required this.marineExpiresAt,
    required this.atmosphericLastModified,
    required this.marineLastModified,
    required this.atmospheric,
    required this.marine,
    this.sun = const {},
    this.moon = const {},
    this.alerts = const [],
  });

  bool get hasMarineData => marine.isNotEmpty;
  bool get hasSunData => sun.isNotEmpty;
  bool get hasActiveAlerts => alerts.isNotEmpty;

  /// The highest-severity active alert, or null if none. Used by the
  /// summary-row banner where only one alert fits.
  MetAlert? get topAlert {
    if (alerts.isEmpty) return null;
    return alerts.reduce(
      (a, b) => a.level.index >= b.level.index ? a : b,
    );
  }

  /// True when every fetched side's `Expires` lies after [now]. A marine side
  /// that was never fetched (`marineExpiresAt == null`) doesn't gate freshness.
  bool isFreshAt(DateTime now) {
    if (!atmosphericExpiresAt.isAfter(now)) return false;
    if (marineExpiresAt != null && !marineExpiresAt!.isAfter(now)) return false;
    return true;
  }

  bool get isFresh => isFreshAt(DateTime.now().toUtc());

  AtmosphericPoint? get currentAtmospheric =>
      atmospheric.isEmpty ? null : atmospheric.first;

  MarinePoint? get currentMarine => marine.isEmpty ? null : marine.first;

  /// Group atmospheric points by local date and compute per-day min/max +
  /// midday symbol.
  List<DailySummary> dailySummaries() {
    if (atmospheric.isEmpty) return const [];
    final byDay = <DateTime, List<AtmosphericPoint>>{};
    for (final p in atmospheric) {
      final local = p.timeUtc.toLocal();
      final day = DateTime(local.year, local.month, local.day);
      byDay.putIfAbsent(day, () => []).add(p);
    }
    final keys = byDay.keys.toList()..sort();
    return [
      for (final day in keys)
        _summaryFor(day, byDay[day]!),
    ];
  }

  DailySummary _summaryFor(DateTime day, List<AtmosphericPoint> points) {
    double minT = double.infinity;
    double maxT = double.negativeInfinity;
    double precip = 0;
    AtmosphericPoint? midday;
    int? bestDiff;
    for (final p in points) {
      if (p.airTemperatureC < minT) minT = p.airTemperatureC;
      if (p.airTemperatureC > maxT) maxT = p.airTemperatureC;
      if (p.precipitation1hMm != null) precip += p.precipitation1hMm!;
      final localHour = p.timeUtc.toLocal().hour;
      final diff = (localHour - 12).abs();
      if (bestDiff == null || diff < bestDiff) {
        bestDiff = diff;
        midday = p;
      }
    }
    return DailySummary(
      date: day,
      minTempC: minT,
      maxTempC: maxT,
      middaySymbol: midday?.symbol12h ?? midday?.symbol6h ?? midday?.symbol1h,
      precipitationTotalMm: precip,
    );
  }

  WeatherForecast copyWith({
    LatLng? position,
    DateTime? fetchedAt,
    DateTime? atmosphericExpiresAt,
    DateTime? marineExpiresAt,
    String? atmosphericLastModified,
    String? marineLastModified,
    List<AtmosphericPoint>? atmospheric,
    List<MarinePoint>? marine,
    Map<DateTime, SunEvent>? sun,
    Map<DateTime, MoonEvent>? moon,
    List<MetAlert>? alerts,
  }) {
    return WeatherForecast(
      position: position ?? this.position,
      fetchedAt: fetchedAt ?? this.fetchedAt,
      atmosphericExpiresAt:
          atmosphericExpiresAt ?? this.atmosphericExpiresAt,
      marineExpiresAt: marineExpiresAt ?? this.marineExpiresAt,
      atmosphericLastModified:
          atmosphericLastModified ?? this.atmosphericLastModified,
      marineLastModified: marineLastModified ?? this.marineLastModified,
      atmospheric: atmospheric ?? this.atmospheric,
      marine: marine ?? this.marine,
      sun: sun ?? this.sun,
      moon: moon ?? this.moon,
      alerts: alerts ?? this.alerts,
    );
  }
}
