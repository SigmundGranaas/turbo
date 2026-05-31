/// Typed view onto the `packrafting` slot in [ActivityAnalysis.kindSlices].
/// Surfaces the current river-flow snapshot, its trend, the user's
/// stored runnable window, and the day-of-year percentile (when the
/// snapshot store has enough history).
class PackraftingAnalysisExtras {
  final double? currentCumecs;
  final String? trend;
  final double? userMinCumecs;
  final double? userMaxCumecs;
  final double? percentile;

  const PackraftingAnalysisExtras({
    required this.currentCumecs,
    required this.trend,
    required this.userMinCumecs,
    required this.userMaxCumecs,
    required this.percentile,
  });

  static PackraftingAnalysisExtras? tryParse(Object? raw) {
    if (raw is! Map<String, dynamic>) return null;
    return PackraftingAnalysisExtras(
      currentCumecs: (raw['currentCumecs'] as num?)?.toDouble(),
      trend: raw['trend'] as String?,
      userMinCumecs: (raw['userMinCumecs'] as num?)?.toDouble(),
      userMaxCumecs: (raw['userMaxCumecs'] as num?)?.toDouble(),
      percentile: (raw['percentile'] as num?)?.toDouble(),
    );
  }
}
