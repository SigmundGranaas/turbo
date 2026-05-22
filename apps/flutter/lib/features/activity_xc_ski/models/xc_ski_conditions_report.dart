import 'package:turbo/features/activity_fishing/api.dart' show WeatherSlice;

class XcSkiConditionsReport {
  final String activityId;
  final DateTime validAt;
  final DateTime fetchedAt;
  final WeatherSlice weather;

  /// Hours since the last grooming pass per the live feed (Skisporet).
  /// Null when no live feed is wired up for this trail.
  final int? liveGroomingHoursAgo;

  final int? score;
  final String rationale;

  const XcSkiConditionsReport({
    required this.activityId,
    required this.validAt,
    required this.fetchedAt,
    required this.weather,
    required this.liveGroomingHoursAgo,
    required this.score,
    required this.rationale,
  });

  factory XcSkiConditionsReport.fromJson(Map<String, dynamic> json) =>
      XcSkiConditionsReport(
        activityId: json['activityId'] as String,
        validAt: DateTime.parse(json['validAt'] as String),
        fetchedAt: DateTime.parse(json['fetchedAt'] as String),
        weather: WeatherSlice.fromJson(json['weather'] as Map<String, dynamic>),
        liveGroomingHoursAgo: (json['liveGroomingHoursAgo'] as num?)?.toInt(),
        score: (json['score'] as num?)?.toInt(),
        rationale: json['rationale'] as String,
      );
}
