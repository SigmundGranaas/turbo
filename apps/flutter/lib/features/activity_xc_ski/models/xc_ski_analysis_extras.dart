/// Typed view onto the `xc_ski` slot in [ActivityAnalysis.kindSlices].
/// Server-side this is the JsonElement the XcSkiOrchestrator emits — the
/// predicted wax band is the only payload today; more fields land as the
/// synthesiser grows.
class XcSkiAnalysisExtras {
  final String? predictedWax;

  const XcSkiAnalysisExtras({required this.predictedWax});

  /// Returns `null` when [raw] is missing or malformed. The conditions
  /// panel renders nothing for the extras row in that case.
  static XcSkiAnalysisExtras? tryParse(Object? raw) {
    if (raw is! Map<String, dynamic>) return null;
    final wax = raw['predictedWax'];
    if (wax is! String?) return null;
    return XcSkiAnalysisExtras(predictedWax: wax);
  }
}
