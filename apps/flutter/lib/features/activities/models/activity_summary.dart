import 'activity_geometry.dart';

/// Cross-kind read-model entry surfaced by the server's
/// `/api/activities/summaries/*` endpoints. The shell uses these to paint
/// the map; tapping one dispatches to that kind's detail screen.
class ActivitySummary {
  final String id;
  final String kind;
  final String name;
  final ActivityGeometry geometry;
  final String iconKey;
  final String? colorHex;
  final DateTime updatedAt;
  final int version;

  /// 0-100 score from the last orchestrator analysis the server wrote
  /// back. Null when no recent score is known (a fresh activity or one
  /// the snapshotter hasn't reached yet). The map layer uses it to
  /// shade the marker halo.
  final int? summaryScore;
  final DateTime? summaryScoreAt;
  final String? topDriverLabel;

  const ActivitySummary({
    required this.id,
    required this.kind,
    required this.name,
    required this.geometry,
    required this.iconKey,
    required this.colorHex,
    required this.updatedAt,
    required this.version,
    this.summaryScore,
    this.summaryScoreAt,
    this.topDriverLabel,
  });

  factory ActivitySummary.fromJson(Map<String, dynamic> json) => ActivitySummary(
    id: json['id'] as String,
    kind: json['kind'] as String,
    name: json['name'] as String,
    geometry: ActivityGeometry.fromServer(
      wkt: json['geometryWkt'] as String,
      geometryKind: json['geometryKind'] as String,
    ),
    iconKey: json['iconKey'] as String,
    colorHex: json['colorHex'] as String?,
    updatedAt: DateTime.parse(json['updatedAt'] as String),
    version: (json['version'] as num).toInt(),
    summaryScore: (json['summaryScore'] as num?)?.toInt(),
    summaryScoreAt: json['summaryScoreAt'] is String
        ? DateTime.parse(json['summaryScoreAt'] as String)
        : null,
    topDriverLabel: json['topDriverLabel'] as String?,
  );
}

/// Tombstone for delta sync.
class ActivitySummaryTombstone {
  final String id;
  final String kind;
  final DateTime deletedAt;
  final int version;
  const ActivitySummaryTombstone({
    required this.id,
    required this.kind,
    required this.deletedAt,
    required this.version,
  });

  factory ActivitySummaryTombstone.fromJson(Map<String, dynamic> json) =>
      ActivitySummaryTombstone(
        id: json['id'] as String,
        kind: json['kind'] as String,
        deletedAt: DateTime.parse(json['deletedAt'] as String),
        version: (json['version'] as num).toInt(),
      );
}
