import 'package:flutter/material.dart';

import '../../models/activity_analysis.dart';
import 'driver_card.dart';
import 'provenance_footer.dart';
import 'score_hero.dart';
import 'suggested_windows_strip.dart';
import 'warning_banner.dart';

/// The whole detail surface for an [ActivityAnalysis]. Kinds compose this
/// inside their own panel/screen wrapper, optionally injecting
/// kind-specific extras between the warnings and the driver list (e.g.
/// xc ski's grooming chip, backcountry's per-aspect loading widget).
///
/// Layout principle: forecast first. Score, windows, warnings, drivers,
/// extras. Static metadata is the caller's responsibility — it lives
/// below the fold on the wrapping screen.
class AnalysisSurface extends StatelessWidget {
  final ActivityAnalysis analysis;
  final Color tintColor;

  /// Optional kind-specific widgets rendered between the warning banners
  /// and the driver list. Typically a single condensed status row.
  final List<Widget> extrasBetweenWarningsAndDrivers;

  /// Optional kind-specific widgets rendered after the driver list and
  /// before the provenance footer. Use for richer kind visualisations
  /// (per-aspect ATES breakdown, hourly flow chart, etc.).
  final List<Widget> extrasBelowDrivers;

  const AnalysisSurface({
    super.key,
    required this.analysis,
    required this.tintColor,
    this.extrasBetweenWarningsAndDrivers = const [],
    this.extrasBelowDrivers = const [],
  });

  @override
  Widget build(BuildContext context) {
    final sortedDrivers = [...analysis.drivers]
      ..sort((a, b) => b.weight.compareTo(a.weight));
    final sortedWarnings = [...analysis.warnings]
      ..sort((a, b) => _severityRank(b.severity) - _severityRank(a.severity));

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ScoreHero(
          score: analysis.score,
          confidence: analysis.confidence,
          rationale: analysis.rationale,
          tintColor: tintColor,
        ),
        SuggestedWindowsStrip(
          windows: analysis.suggestedWindows,
          tintColor: tintColor,
        ),
        for (final w in sortedWarnings) WarningBanner(warning: w),
        ...extrasBetweenWarningsAndDrivers,
        for (final d in sortedDrivers) DriverCard(driver: d, tintColor: tintColor),
        ...extrasBelowDrivers,
        ProvenanceFooter(provenance: analysis.provenance),
      ],
    );
  }

  static int _severityRank(WarningSeverity s) => switch (s) {
        WarningSeverity.danger => 3,
        WarningSeverity.caution => 2,
        WarningSeverity.info => 1,
      };
}
