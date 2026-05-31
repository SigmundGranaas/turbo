import 'package:flutter/material.dart';

import '../../models/activity_analysis.dart';

/// Big-number score at the top of the detail screen. Shows the qualitative
/// band, a confidence pill, and the orchestrator's one-sentence rationale.
/// Renders honestly when the orchestrator could not produce a score
/// (`null` → big em-dash plus "Not enough signal").
class ScoreHero extends StatelessWidget {
  final int? score;
  final ScoreConfidence confidence;
  final String rationale;
  final Color tintColor;

  const ScoreHero({
    super.key,
    required this.score,
    required this.confidence,
    required this.rationale,
    required this.tintColor,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scoreText = score?.toString() ?? '—';
    final bandLabel = _qualitativeBandFor(score);

    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 16, 20, 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Text(
                scoreText,
                style: theme.textTheme.displayMedium?.copyWith(
                  color: tintColor,
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      bandLabel,
                      style: theme.textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 4),
                    _ConfidenceChip(confidence: confidence),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Text(
            rationale,
            style: theme.textTheme.bodyMedium,
          ),
        ],
      ),
    );
  }

  static String _qualitativeBandFor(int? score) {
    if (score == null) return 'Not enough signal';
    if (score >= 80) return 'Great today';
    if (score >= 60) return 'Good — go with intent';
    if (score >= 40) return 'Marginal — pick your window';
    if (score >= 20) return 'Tough — consider waiting';
    return 'Avoid today';
  }
}

class _ConfidenceChip extends StatelessWidget {
  final ScoreConfidence confidence;
  const _ConfidenceChip({required this.confidence});

  @override
  Widget build(BuildContext context) {
    final (label, color) = switch (confidence) {
      ScoreConfidence.high => ('High confidence', Colors.green.shade700),
      ScoreConfidence.medium => ('Medium confidence', Colors.orange.shade800),
      ScoreConfidence.low => ('Low confidence — bring your own read', Colors.deepOrange.shade700),
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Text(
        label,
        style: TextStyle(
          color: color,
          fontSize: 11,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}
