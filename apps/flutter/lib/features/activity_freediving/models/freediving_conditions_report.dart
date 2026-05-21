import 'package:turbo/features/activity_fishing/api.dart' show WeatherSlice;

class FreedivingConditionsReport {
  final String activityId;
  final DateTime validAt;
  final DateTime fetchedAt;
  final WeatherSlice weather;
  final String? seaStateSummary;
  final int? score;
  final String rationale;

  const FreedivingConditionsReport({
    required this.activityId,
    required this.validAt,
    required this.fetchedAt,
    required this.weather,
    required this.seaStateSummary,
    required this.score,
    required this.rationale,
  });

  factory FreedivingConditionsReport.fromJson(Map<String, dynamic> json) =>
      FreedivingConditionsReport(
        activityId: json['activityId'] as String,
        validAt: DateTime.parse(json['validAt'] as String),
        fetchedAt: DateTime.parse(json['fetchedAt'] as String),
        weather: WeatherSlice.fromJson(json['weather'] as Map<String, dynamic>),
        seaStateSummary: json['seaStateSummary'] as String?,
        score: (json['score'] as num?)?.toInt(),
        rationale: json['rationale'] as String,
      );
}
