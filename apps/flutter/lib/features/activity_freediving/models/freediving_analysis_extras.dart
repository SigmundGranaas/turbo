/// Typed view onto the `freediving` slot in [ActivityAnalysis.kindSlices].
/// Carries the computed visibility forecast (the field the original
/// critique zeroed in on — "typicalVisibilityMeters is bullshit") and
/// the tide info.
class FreedivingAnalysisExtras {
  final VizForecast? vizForecast;
  final TideInfo? tide;

  /// Surfaces the (now-deprecated) user-entered field so the UI can
  /// show "you said vs we estimate" if it wants. Null once the field
  /// is removed.
  final double? storedTypicalVizM;

  const FreedivingAnalysisExtras({
    required this.vizForecast,
    required this.tide,
    required this.storedTypicalVizM,
  });

  static FreedivingAnalysisExtras? tryParse(Object? raw) {
    if (raw is! Map<String, dynamic>) return null;
    final vizRaw = raw['vizForecast'];
    final tideRaw = raw['tide'];
    return FreedivingAnalysisExtras(
      vizForecast: vizRaw is Map<String, dynamic> ? VizForecast.fromJson(vizRaw) : null,
      tide: tideRaw is Map<String, dynamic> ? TideInfo.fromJson(tideRaw) : null,
      storedTypicalVizM: (raw['storedTypicalVizM'] as num?)?.toDouble(),
    );
  }
}

class VizForecast {
  final double low;
  final double high;
  final String? direction;
  final double confidence;

  const VizForecast({
    required this.low,
    required this.high,
    required this.direction,
    required this.confidence,
  });

  factory VizForecast.fromJson(Map<String, dynamic> json) => VizForecast(
        low: (json['low'] as num?)?.toDouble() ?? 0,
        high: (json['high'] as num?)?.toDouble() ?? 0,
        direction: json['direction'] as String?,
        confidence: (json['confidence'] as num?)?.toDouble() ?? 0,
      );
}

class TideInfo {
  final double? heightM;
  final String? summary;
  const TideInfo({required this.heightM, required this.summary});

  factory TideInfo.fromJson(Map<String, dynamic> json) => TideInfo(
        heightM: (json['heightM'] as num?)?.toDouble(),
        summary: json['summary'] as String?,
      );
}
