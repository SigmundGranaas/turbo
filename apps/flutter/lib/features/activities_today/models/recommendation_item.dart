import 'package:turbo/features/activities/api.dart';

/// One row of the Today screen. Mirrors the server's RecommendationItem
/// — the score the orchestrator emitted via QuickScoreAsync, plus a
/// distance and a short headline.
class RecommendationItem {
  final String sourceKind; // "own_activity" | "discovered_*"
  final String kind;       // "xc_ski", "backcountry_ski", etc.
  final String? activityId;
  final String name;
  final String geometryWkt;
  final int? score;
  final ScoreConfidence confidence;
  final String headline;
  final String? topDriverLabel;
  final AnalysisTimeWindow? suggestedWindow;
  final List<AnalysisWarning> topWarnings;
  final double distanceM;

  const RecommendationItem({
    required this.sourceKind,
    required this.kind,
    required this.activityId,
    required this.name,
    required this.geometryWkt,
    required this.score,
    required this.confidence,
    required this.headline,
    required this.topDriverLabel,
    required this.suggestedWindow,
    required this.topWarnings,
    required this.distanceM,
  });

  factory RecommendationItem.fromJson(Map<String, dynamic> json) =>
      RecommendationItem(
        sourceKind: json['sourceKind'] as String,
        kind: json['kind'] as String,
        activityId: json['activityId'] as String?,
        name: json['name'] as String,
        geometryWkt: json['geometryWkt'] as String,
        score: (json['score'] as num?)?.toInt(),
        confidence: ScoreConfidence.fromJson(json['confidence']),
        headline: json['headline'] as String,
        topDriverLabel: json['topDriverLabel'] as String?,
        suggestedWindow: json['suggestedWindow'] == null
            ? null
            : AnalysisTimeWindow.fromJson(
                json['suggestedWindow'] as Map<String, dynamic>),
        topWarnings: ((json['topWarnings'] as List<dynamic>?) ?? const [])
            .map((w) => AnalysisWarning.fromJson(w as Map<String, dynamic>))
            .toList(growable: false),
        distanceM: (json['distanceM'] as num?)?.toDouble() ?? 0,
      );
}

class RecommendationsResponse {
  final List<RecommendationItem> items;
  final DateTime serverTime;

  const RecommendationsResponse({required this.items, required this.serverTime});

  factory RecommendationsResponse.fromJson(Map<String, dynamic> json) =>
      RecommendationsResponse(
        items: ((json['items'] as List<dynamic>?) ?? const [])
            .map((i) => RecommendationItem.fromJson(i as Map<String, dynamic>))
            .toList(growable: false),
        serverTime: DateTime.parse(json['serverTime'] as String),
      );
}
