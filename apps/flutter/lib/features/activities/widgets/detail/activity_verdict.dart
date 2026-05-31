import 'package:flutter/material.dart';

import 'condition_palette.dart';

/// Tone of a verdict — drives the left tone-bar color and the optional
/// circular score badge background.
enum VerdictTone { good, caution, stop, warn, neutral }

/// Semantic state of a verdict — drives a11y semantics (live region
/// announcements, status role) and whether the score badge is shown.
/// A `.loading` verdict announces "fetching"; a `.fallback` verdict
/// announces "unavailable"; only `.ready` carries a real judgment.
enum VerdictState { loading, fallback, ready }

/// The verdict card. One headline that answers "should I do this
/// today?", optional supporting line, optional pill badge (e.g.
/// "LEVEL 3"), optional 0–100 score circle.
///
/// Renders in `surfaceContainer` with a 6px tone bar on the left. The
/// rest of the card stays neutral — tone is encoded only in the bar
/// and optional badge/score color so the verdict reads as judgment,
/// not noise.
///
/// **Renders synchronously.** All four states (loading / ready /
/// fallback / error) are *just verdicts* with different copy, tone,
/// and a11y semantics — there is no spinner or shimmer pathway. This
/// is the architectural fix for the "spins forever" complaint.
class ActivityVerdict extends StatelessWidget {
  final String headline;
  final String? support;
  final String? badge;
  final int? score;
  final VerdictTone tone;
  final VerdictState state;

  const ActivityVerdict({
    super.key,
    required this.headline,
    this.support,
    this.badge,
    this.score,
    this.tone = VerdictTone.neutral,
    this.state = VerdictState.ready,
  });

  /// Quiet placeholder for the analysis-pending state.
  const ActivityVerdict.loading({
    super.key,
    this.headline = 'Fetching conditions…',
    this.support = 'Activity loaded · conditions arrive in a moment.',
  })  : badge = null,
        score = null,
        tone = VerdictTone.neutral,
        state = VerdictState.loading;

  /// Graceful fallback when the analysis call failed or returned a
  /// score-less snapshot. Carries no number.
  const ActivityVerdict.fallback({
    super.key,
    required this.support,
    this.headline = 'Conditions unavailable',
  })  : badge = null,
        score = null,
        tone = VerdictTone.neutral,
        state = VerdictState.fallback;

  /// Builds a ready verdict from an orchestrator score. Centralises
  /// the score-band → tone + headline policy so it lives in one place
  /// instead of 6.
  factory ActivityVerdict.fromScore({
    Key? key,
    required int? score,
    required String rationale,
    String? badge,
  }) {
    final tone = toneFromScore(score);
    final headline =
        score != null ? headlineForScore(score) : 'Conditions snapshot';
    return ActivityVerdict(
      key: key,
      headline: headline,
      support: rationale.isEmpty ? null : rationale,
      badge: badge,
      score: score,
      tone: tone,
      state: VerdictState.ready,
    );
  }

  /// Score → headline mapping. Single source of truth.
  static String headlineForScore(int score) {
    if (score >= 80) return 'Great today';
    if (score >= 60) return 'Good — go';
    if (score >= 40) return 'Marginal — pick your window';
    if (score >= 20) return 'Tough — consider waiting';
    return 'Avoid today';
  }

  /// Score → tone mapping. Single source of truth.
  static VerdictTone toneFromScore(int? score) {
    if (score == null) return VerdictTone.neutral;
    if (score >= 70) return VerdictTone.good;
    if (score >= 40) return VerdictTone.caution;
    return VerdictTone.stop;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final toneColor = _toneColor(tone, theme);
    // Neutral tone means "no judgment" — don't render a number there
    // even if one was passed, since a grey-filled circle clashes with
    // surrounding muted copy and would suggest a score where there
    // really isn't one.
    final showScore = score != null && tone != VerdictTone.neutral;
    return Semantics(
      container: true,
      liveRegion: state == VerdictState.loading,
      label: switch (state) {
        VerdictState.loading => 'Fetching conditions',
        VerdictState.fallback => 'Conditions unavailable, $support',
        VerdictState.ready => support == null
            ? headline
            : '$headline. $support${score != null ? ', score $score' : ''}',
      },
      child: ExcludeSemantics(
        child: Container(
          margin: const EdgeInsets.only(top: 12),
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: theme.colorScheme.surfaceContainer,
            borderRadius: BorderRadius.circular(14),
          ),
          child: IntrinsicHeight(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Container(
                  width: 6,
                  decoration: BoxDecoration(
                    color: toneColor,
                    borderRadius: BorderRadius.circular(3),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Row(
                        crossAxisAlignment: CrossAxisAlignment.center,
                        children: [
                          Flexible(
                            child: Text(
                              headline,
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.titleMedium
                                  ?.copyWith(fontWeight: FontWeight.w500),
                            ),
                          ),
                          if (badge != null) ...[
                            const SizedBox(width: 8),
                            _BadgePill(text: badge!, color: toneColor),
                          ],
                        ],
                      ),
                      if (support != null) ...[
                        const SizedBox(height: 2),
                        Text(
                          support!,
                          maxLines: 3,
                          overflow: TextOverflow.ellipsis,
                          style: theme.textTheme.bodySmall?.copyWith(
                              color: theme.colorScheme.onSurfaceVariant),
                        ),
                      ],
                    ],
                  ),
                ),
                if (showScore) ...[
                  const SizedBox(width: 12),
                  Center(
                    child: Container(
                      width: 36,
                      height: 36,
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: toneColor,
                      ),
                      child: Text(
                        '$score',
                        style: const TextStyle(
                          fontSize: 13,
                          height: 16 / 13,
                          fontWeight: FontWeight.w700,
                          color: Colors.white,
                        ),
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }

  static Color _toneColor(VerdictTone tone, ThemeData theme) => switch (tone) {
        VerdictTone.good => ConditionPalette.good,
        VerdictTone.caution => ConditionPalette.caution,
        VerdictTone.stop => ConditionPalette.stop,
        VerdictTone.warn => ConditionPalette.warn,
        VerdictTone.neutral => theme.colorScheme.onSurfaceVariant,
      };
}

class _BadgePill extends StatelessWidget {
  final String text;
  final Color color;
  const _BadgePill({required this.text, required this.color});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.16),
        borderRadius: BorderRadius.circular(9999),
      ),
      child: Text(
        text,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: TextStyle(
          fontSize: 11,
          height: 14 / 11,
          fontWeight: FontWeight.w500,
          letterSpacing: 0.3,
          color: color,
        ),
      ),
    );
  }
}
