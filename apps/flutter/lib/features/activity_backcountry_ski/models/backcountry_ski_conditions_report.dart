import 'package:turbo/features/activity_fishing/api.dart' show WeatherSlice;

/// Typed conditions report returned by
/// `/api/activities/backcountry-ski/{id}/conditions`. Mirrors the
/// server's [BackcountrySkiConditionsReport]. Reuses the
/// [WeatherSlice] exported from the fishing kind — same data shape,
/// same client-side parser. Each kind owns its scoring + rationale.
class BackcountrySkiConditionsReport {
  final String activityId;
  final DateTime validAt;
  final DateTime fetchedAt;
  final WeatherSlice weather;

  /// Varsom avalanche danger level 1–5 if available. Null until the
  /// VarsomAvalancheProvider lands server-side.
  final int? avalancheLevel;

  /// Short summary from Varsom. Null until the provider lands.
  final String? avalancheSummary;

  final int? score;
  final String rationale;

  const BackcountrySkiConditionsReport({
    required this.activityId,
    required this.validAt,
    required this.fetchedAt,
    required this.weather,
    required this.avalancheLevel,
    required this.avalancheSummary,
    required this.score,
    required this.rationale,
  });

  factory BackcountrySkiConditionsReport.fromJson(Map<String, dynamic> json) =>
      BackcountrySkiConditionsReport(
        activityId: json['activityId'] as String,
        validAt: DateTime.parse(json['validAt'] as String),
        fetchedAt: DateTime.parse(json['fetchedAt'] as String),
        weather: WeatherSlice.fromJson(json['weather'] as Map<String, dynamic>),
        avalancheLevel: (json['avalancheLevel'] as num?)?.toInt(),
        avalancheSummary: json['avalancheSummary'] as String?,
        score: (json['score'] as num?)?.toInt(),
        rationale: json['rationale'] as String,
      );
}
