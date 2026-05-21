import 'package:turbo/features/activity_fishing/api.dart' show WeatherSlice;

/// Server-derived hiking conditions report. Reuses the shared
/// [WeatherSlice] parser from the fishing kind (same wire shape; one
/// Dart class for all kinds).
class HikingConditionsReport {
  final String activityId;
  final DateTime validAt;
  final DateTime fetchedAt;
  final WeatherSlice weather;
  final int? score;
  final String rationale;

  const HikingConditionsReport({
    required this.activityId,
    required this.validAt,
    required this.fetchedAt,
    required this.weather,
    required this.score,
    required this.rationale,
  });

  factory HikingConditionsReport.fromJson(Map<String, dynamic> json) =>
      HikingConditionsReport(
        activityId: json['activityId'] as String,
        validAt: DateTime.parse(json['validAt'] as String),
        fetchedAt: DateTime.parse(json['fetchedAt'] as String),
        weather: WeatherSlice.fromJson(json['weather'] as Map<String, dynamic>),
        score: (json['score'] as num?)?.toInt(),
        rationale: json['rationale'] as String,
      );
}
