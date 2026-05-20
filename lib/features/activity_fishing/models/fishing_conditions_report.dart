/// Typed report returned by `/api/activities/fishing/{id}/conditions`.
/// Mirrors the server's [FishingConditionsReport]; every field has a
/// name and a unit, no maps-of-strings.
class FishingConditionsReport {
  final String activityId;
  final DateTime validAt;
  final DateTime fetchedAt;
  final WeatherSlice weather;
  final int? score;
  final String rationale;

  const FishingConditionsReport({
    required this.activityId,
    required this.validAt,
    required this.fetchedAt,
    required this.weather,
    required this.score,
    required this.rationale,
  });

  factory FishingConditionsReport.fromJson(Map<String, dynamic> json) =>
      FishingConditionsReport(
        activityId: json['activityId'] as String,
        validAt: DateTime.parse(json['validAt'] as String),
        fetchedAt: DateTime.parse(json['fetchedAt'] as String),
        weather: WeatherSlice.fromJson(json['weather'] as Map<String, dynamic>),
        score: (json['score'] as num?)?.toInt(),
        rationale: json['rationale'] as String,
      );
}

/// Typed weather snapshot for one (location, time). Cached server-side
/// per (grid cell, hour) — clients render verbatim.
class WeatherSlice {
  final DateTime validAt;
  final double airTemperatureCelsius;
  final double airPressureHpa;
  final double relativeHumidityPct;
  final double cloudCoveragePct;
  final double windSpeedMs;
  final double? windGustMs;
  final double windFromDegrees;
  final double? precipitationNext1hMm;
  final double? precipitationNext6hMm;
  final String? symbolCode;

  const WeatherSlice({
    required this.validAt,
    required this.airTemperatureCelsius,
    required this.airPressureHpa,
    required this.relativeHumidityPct,
    required this.cloudCoveragePct,
    required this.windSpeedMs,
    required this.windGustMs,
    required this.windFromDegrees,
    required this.precipitationNext1hMm,
    required this.precipitationNext6hMm,
    required this.symbolCode,
  });

  factory WeatherSlice.fromJson(Map<String, dynamic> json) => WeatherSlice(
        validAt: DateTime.parse(json['validAt'] as String),
        airTemperatureCelsius: (json['airTemperatureCelsius'] as num).toDouble(),
        airPressureHpa: (json['airPressureHpa'] as num).toDouble(),
        relativeHumidityPct: (json['relativeHumidityPct'] as num).toDouble(),
        cloudCoveragePct: (json['cloudCoveragePct'] as num).toDouble(),
        windSpeedMs: (json['windSpeedMs'] as num).toDouble(),
        windGustMs: (json['windGustMs'] as num?)?.toDouble(),
        windFromDegrees: (json['windFromDegrees'] as num).toDouble(),
        precipitationNext1hMm: (json['precipitationNext1hMm'] as num?)?.toDouble(),
        precipitationNext6hMm: (json['precipitationNext6hMm'] as num?)?.toDouble(),
        symbolCode: json['symbolCode'] as String?,
      );
}
