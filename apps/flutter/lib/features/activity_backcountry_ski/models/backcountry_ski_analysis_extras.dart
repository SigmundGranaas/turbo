/// Typed view onto the `backcountry_ski` slot in
/// [ActivityAnalysis.kindSlices]. The orchestrator emits per-aspect
/// wind-loading info here so the conditions panel can show which aspects
/// to avoid today without needing a full ATES diagram.
class BackcountrySkiAnalysisExtras {
  final double? windFromDegrees;
  final String? leeAspect;
  final double? loadingFactor; // 0..1, where 1 = heaviest loading
  final List<AspectLoading> perAspect;

  const BackcountrySkiAnalysisExtras({
    required this.windFromDegrees,
    required this.leeAspect,
    required this.loadingFactor,
    required this.perAspect,
  });

  static BackcountrySkiAnalysisExtras? tryParse(Object? raw) {
    if (raw is! Map<String, dynamic>) return null;
    final perAspectRaw = raw['perAspect'];
    final perAspect = perAspectRaw is List
        ? perAspectRaw
            .whereType<Map<String, dynamic>>()
            .map(AspectLoading.fromJson)
            .toList(growable: false)
        : <AspectLoading>[];
    return BackcountrySkiAnalysisExtras(
      windFromDegrees: (raw['windFromDegrees'] as num?)?.toDouble(),
      leeAspect: raw['leeAspect'] as String?,
      loadingFactor: (raw['loadingFactor'] as num?)?.toDouble(),
      perAspect: perAspect,
    );
  }
}

class AspectLoading {
  final String aspect;
  final double fraction;
  final double loadedFractionOfFraction;

  const AspectLoading({
    required this.aspect,
    required this.fraction,
    required this.loadedFractionOfFraction,
  });

  factory AspectLoading.fromJson(Map<String, dynamic> json) => AspectLoading(
        aspect: json['aspect'] as String,
        fraction: (json['fraction'] as num?)?.toDouble() ?? 0,
        loadedFractionOfFraction:
            (json['loadedFractionOfFraction'] as num?)?.toDouble() ?? 0,
      );
}
