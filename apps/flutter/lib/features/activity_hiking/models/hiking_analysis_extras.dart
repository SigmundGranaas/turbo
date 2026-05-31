/// Typed view onto the `hiking` slot in [ActivityAnalysis.kindSlices].
/// Surfaces the DEM-derived ascent/descent/length the geo-context
/// service computed, plus above-treeline exposure fraction — useful
/// when the user-stored values are missing or out of date.
class HikingAnalysisExtras {
  final double? ascentMDerived;
  final double? descentMDerived;
  final double? lengthMDerived;
  final double? aboveTreelineFraction;
  final double? estimatedHoursStored;

  const HikingAnalysisExtras({
    required this.ascentMDerived,
    required this.descentMDerived,
    required this.lengthMDerived,
    required this.aboveTreelineFraction,
    required this.estimatedHoursStored,
  });

  static HikingAnalysisExtras? tryParse(Object? raw) {
    if (raw is! Map<String, dynamic>) return null;
    return HikingAnalysisExtras(
      ascentMDerived: (raw['ascentMDerived'] as num?)?.toDouble(),
      descentMDerived: (raw['descentMDerived'] as num?)?.toDouble(),
      lengthMDerived: (raw['lengthMDerived'] as num?)?.toDouble(),
      aboveTreelineFraction: (raw['aboveTreelineFractionM'] as num?)?.toDouble(),
      estimatedHoursStored: (raw['estimatedHoursStored'] as num?)?.toDouble(),
    );
  }
}
