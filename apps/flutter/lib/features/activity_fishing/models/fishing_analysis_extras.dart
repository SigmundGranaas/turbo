/// Typed view onto the `fishing` slot in [ActivityAnalysis.kindSlices].
/// Carries the predicted bite window (computed from solunar majors
/// intersected with weather-acceptable hours).
class FishingAnalysisExtras {
  final BiteWindow? biteWindow;

  const FishingAnalysisExtras({required this.biteWindow});

  static FishingAnalysisExtras? tryParse(Object? raw) {
    if (raw is! Map<String, dynamic>) return null;
    final bw = raw['biteWindow'];
    return FishingAnalysisExtras(
      biteWindow: bw is Map<String, dynamic> ? BiteWindow.fromJson(bw) : null,
    );
  }
}

class BiteWindow {
  final DateTime start;
  final DateTime end;
  final String? rationale;

  const BiteWindow({required this.start, required this.end, required this.rationale});

  factory BiteWindow.fromJson(Map<String, dynamic> json) => BiteWindow(
        start: DateTime.parse(json['start'] as String),
        end: DateTime.parse(json['end'] as String),
        rationale: json['rationale'] as String?,
      );
}
