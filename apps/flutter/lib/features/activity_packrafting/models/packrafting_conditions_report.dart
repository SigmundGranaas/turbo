import 'package:turbo/features/activity_fishing/api.dart' show WeatherSlice;

class PackraftingConditionsReport {
  final String activityId;
  final DateTime validAt;
  final DateTime fetchedAt;
  final WeatherSlice weather;

  /// Current flow in m³/s from the linked NVE station. Null when the
  /// activity has no station configured or the NVE provider isn't wired.
  final double? currentFlowCumecs;

  /// "rising" / "stable" / "falling", or null if not enough data.
  final String? flowTrend;

  final int? score;
  final String rationale;

  const PackraftingConditionsReport({
    required this.activityId,
    required this.validAt,
    required this.fetchedAt,
    required this.weather,
    required this.currentFlowCumecs,
    required this.flowTrend,
    required this.score,
    required this.rationale,
  });

  factory PackraftingConditionsReport.fromJson(Map<String, dynamic> json) =>
      PackraftingConditionsReport(
        activityId: json['activityId'] as String,
        validAt: DateTime.parse(json['validAt'] as String),
        fetchedAt: DateTime.parse(json['fetchedAt'] as String),
        weather: WeatherSlice.fromJson(json['weather'] as Map<String, dynamic>),
        currentFlowCumecs: (json['currentFlowCumecs'] as num?)?.toDouble(),
        flowTrend: json['flowTrend'] as String?,
        score: (json['score'] as num?)?.toInt(),
        rationale: json['rationale'] as String,
      );
}
