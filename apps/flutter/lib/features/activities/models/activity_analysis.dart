/// Typed mirror of the server's `ActivityAnalysis` record returned from
/// `GET /api/activities/{kind}/{id}/analysis`. Unlike each kind's existing
/// `{Kind}ConditionsReport` — which collapses everything into a score plus
/// a one-line rationale — this surfaces the named drivers behind the
/// score, forecast bands per metric, suggested time windows, warnings,
/// and source provenance so the UI can render *why*, not just *what*.
///
/// Kind-specific structured payloads ride in [kindSlices] and are parsed
/// by tiny per-kind parsers (e.g. `XcSkiAnalysisExtras.fromJson(...)`)
/// kept next to each feature.
class ActivityAnalysis {
  final String activityId;
  final String kind;
  final DateTime validAt;
  final DateTime fetchedAt;
  final int? score;
  final ScoreConfidence confidence;
  final String rationale;
  final List<AnalysisDriver> drivers;
  final List<ForecastBand> bands;
  final List<AnalysisWarning> warnings;
  final List<AnalysisTimeWindow> suggestedWindows;
  final Map<String, Object?> kindSlices;
  final AnalysisProvenance provenance;

  const ActivityAnalysis({
    required this.activityId,
    required this.kind,
    required this.validAt,
    required this.fetchedAt,
    required this.score,
    required this.confidence,
    required this.rationale,
    required this.drivers,
    required this.bands,
    required this.warnings,
    required this.suggestedWindows,
    required this.kindSlices,
    required this.provenance,
  });

  factory ActivityAnalysis.fromJson(Map<String, dynamic> json) =>
      ActivityAnalysis(
        activityId: json['activityId'] as String,
        kind: json['kind'] as String,
        validAt: DateTime.parse(json['validAt'] as String),
        fetchedAt: DateTime.parse(json['fetchedAt'] as String),
        score: (json['score'] as num?)?.toInt(),
        confidence: ScoreConfidence.fromJson(json['confidence']),
        rationale: json['rationale'] as String,
        drivers: ((json['drivers'] as List<dynamic>?) ?? const [])
            .map((d) => AnalysisDriver.fromJson(d as Map<String, dynamic>))
            .toList(growable: false),
        bands: ((json['bands'] as List<dynamic>?) ?? const [])
            .map((b) => ForecastBand.fromJson(b as Map<String, dynamic>))
            .toList(growable: false),
        warnings: ((json['warnings'] as List<dynamic>?) ?? const [])
            .map((w) => AnalysisWarning.fromJson(w as Map<String, dynamic>))
            .toList(growable: false),
        suggestedWindows: ((json['suggestedWindows'] as List<dynamic>?) ?? const [])
            .map((s) => AnalysisTimeWindow.fromJson(s as Map<String, dynamic>))
            .toList(growable: false),
        kindSlices: (json['kindSlices'] as Map<String, dynamic>?) ?? const {},
        provenance: AnalysisProvenance.fromJson(
            (json['provenance'] as Map<String, dynamic>?) ?? const {}),
      );
}

/// One named contributor to the composite score. The UI renders these as
/// expandable cards with a weight bar, confidence dots, optional sparkline
/// (when [band] is non-null), and the per-driver rationale.
class AnalysisDriver {
  final String key;
  final String label;
  final String? unit;
  final double? value;
  final double weight;
  final double confidence;
  final String? direction;
  final ForecastBand? band;
  final String? rationale;

  const AnalysisDriver({
    required this.key,
    required this.label,
    required this.unit,
    required this.value,
    required this.weight,
    required this.confidence,
    required this.direction,
    required this.band,
    required this.rationale,
  });

  factory AnalysisDriver.fromJson(Map<String, dynamic> json) => AnalysisDriver(
        key: json['key'] as String,
        label: json['label'] as String,
        unit: json['unit'] as String?,
        value: (json['value'] as num?)?.toDouble(),
        weight: (json['weight'] as num?)?.toDouble() ?? 0,
        confidence: (json['confidence'] as num?)?.toDouble() ?? 0,
        direction: json['direction'] as String?,
        band: json['band'] == null
            ? null
            : ForecastBand.fromJson(json['band'] as Map<String, dynamic>),
        rationale: json['rationale'] as String?,
      );
}

class ForecastBand {
  final String metric;
  final List<ForecastSample> samples;
  final String? trend;
  final double confidence;

  const ForecastBand({
    required this.metric,
    required this.samples,
    required this.trend,
    required this.confidence,
  });

  factory ForecastBand.fromJson(Map<String, dynamic> json) => ForecastBand(
        metric: json['metric'] as String,
        samples: ((json['samples'] as List<dynamic>?) ?? const [])
            .map((s) => ForecastSample.fromJson(s as Map<String, dynamic>))
            .toList(growable: false),
        trend: json['trend'] as String?,
        confidence: (json['confidence'] as num?)?.toDouble() ?? 0,
      );
}

class ForecastSample {
  final DateTime at;
  final double value;
  final double? lower;
  final double? upper;

  const ForecastSample({
    required this.at,
    required this.value,
    required this.lower,
    required this.upper,
  });

  factory ForecastSample.fromJson(Map<String, dynamic> json) => ForecastSample(
        at: DateTime.parse(json['at'] as String),
        value: (json['value'] as num).toDouble(),
        lower: (json['lower'] as num?)?.toDouble(),
        upper: (json['upper'] as num?)?.toDouble(),
      );
}

class AnalysisTimeWindow {
  final DateTime start;
  final DateTime end;
  final WindowQuality quality;
  final String label;
  final String? reason;

  const AnalysisTimeWindow({
    required this.start,
    required this.end,
    required this.quality,
    required this.label,
    required this.reason,
  });

  factory AnalysisTimeWindow.fromJson(Map<String, dynamic> json) =>
      AnalysisTimeWindow(
        start: DateTime.parse(json['start'] as String),
        end: DateTime.parse(json['end'] as String),
        quality: WindowQuality.fromJson(json['quality']),
        label: json['label'] as String,
        reason: json['reason'] as String?,
      );
}

class AnalysisWarning {
  final String code;
  final WarningSeverity severity;
  final String title;
  final String body;
  final String? sourceUrl;

  const AnalysisWarning({
    required this.code,
    required this.severity,
    required this.title,
    required this.body,
    required this.sourceUrl,
  });

  factory AnalysisWarning.fromJson(Map<String, dynamic> json) =>
      AnalysisWarning(
        code: json['code'] as String,
        severity: WarningSeverity.fromJson(json['severity']),
        title: json['title'] as String,
        body: json['body'] as String,
        sourceUrl: json['sourceUrl'] as String?,
      );
}

class AnalysisProvenance {
  final List<AnalysisSourceHit> sources;
  final int durationMs;

  const AnalysisProvenance({required this.sources, required this.durationMs});

  factory AnalysisProvenance.fromJson(Map<String, dynamic> json) =>
      AnalysisProvenance(
        sources: ((json['sources'] as List<dynamic>?) ?? const [])
            .map((s) => AnalysisSourceHit.fromJson(s as Map<String, dynamic>))
            .toList(growable: false),
        durationMs: (json['durationMs'] as num?)?.toInt() ?? 0,
      );
}

class AnalysisSourceHit {
  final String providerKey;
  final bool ok;
  final bool fromCache;
  final int? ageSeconds;
  final String? error;

  const AnalysisSourceHit({
    required this.providerKey,
    required this.ok,
    required this.fromCache,
    required this.ageSeconds,
    required this.error,
  });

  factory AnalysisSourceHit.fromJson(Map<String, dynamic> json) =>
      AnalysisSourceHit(
        providerKey: json['providerKey'] as String,
        ok: json['ok'] as bool? ?? false,
        fromCache: json['fromCache'] as bool? ?? false,
        ageSeconds: (json['ageSeconds'] as num?)?.toInt(),
        error: json['error'] as String?,
      );
}

enum ScoreConfidence {
  low,
  medium,
  high;

  static ScoreConfidence fromJson(Object? raw) {
    final s = (raw is String) ? raw.toLowerCase() : 'medium';
    return ScoreConfidence.values.firstWhere(
      (e) => e.name == s,
      orElse: () => ScoreConfidence.medium,
    );
  }
}

enum WindowQuality {
  excellent,
  good,
  marginal,
  avoid;

  static WindowQuality fromJson(Object? raw) {
    final s = (raw is String) ? raw.toLowerCase() : 'good';
    return WindowQuality.values.firstWhere(
      (e) => e.name == s,
      orElse: () => WindowQuality.good,
    );
  }
}

enum WarningSeverity {
  info,
  caution,
  danger;

  static WarningSeverity fromJson(Object? raw) {
    final s = (raw is String) ? raw.toLowerCase() : 'info';
    return WarningSeverity.values.firstWhere(
      (e) => e.name == s,
      orElse: () => WarningSeverity.info,
    );
  }
}
