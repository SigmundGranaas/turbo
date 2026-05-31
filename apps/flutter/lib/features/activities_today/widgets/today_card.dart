import 'package:flutter/material.dart';

import 'package:turbo/features/activities/api.dart';

import '../models/recommendation_item.dart';

/// One ranked card on the Today screen. Surfaces score, top driver,
/// distance, top warning. Tap to open the kind's detail screen.
class TodayCard extends StatelessWidget {
  final RecommendationItem item;
  final Color tintColor;
  final VoidCallback? onTap;

  const TodayCard({
    super.key,
    required this.item,
    required this.tintColor,
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(8),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  _ScoreCircle(score: item.score, tintColor: tintColor),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(item.name,
                            style: theme.textTheme.titleMedium?.copyWith(
                                fontWeight: FontWeight.w600)),
                        const SizedBox(height: 2),
                        Text(
                          '${_formatKind(item.kind)} · ${_formatDistance(item.distanceM)}',
                          style: theme.textTheme.bodySmall?.copyWith(
                              color: theme.colorScheme.onSurfaceVariant),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Text(item.headline, style: theme.textTheme.bodyMedium),
              if (item.topDriverLabel != null || item.suggestedWindow != null) ...[
                const SizedBox(height: 8),
                Wrap(
                  spacing: 6,
                  runSpacing: 6,
                  children: [
                    if (item.topDriverLabel != null)
                      _Chip(
                        icon: Icons.insights_outlined,
                        label: item.topDriverLabel!,
                        tint: tintColor,
                      ),
                    if (item.suggestedWindow != null)
                      _Chip(
                        icon: Icons.access_time_rounded,
                        label: item.suggestedWindow!.label,
                        tint: tintColor,
                      ),
                  ],
                ),
              ],
              if (item.topWarnings.isNotEmpty) ...[
                const SizedBox(height: 8),
                for (final w in item.topWarnings.take(1))
                  _WarningSnippet(warning: w),
              ],
            ],
          ),
        ),
      ),
    );
  }

  static String _formatDistance(double m) {
    if (m < 1000) return '${m.round()} m';
    final km = m / 1000;
    return km < 10 ? '${km.toStringAsFixed(1)} km' : '${km.round()} km';
  }

  static String _formatKind(String kind) => switch (kind) {
        'xc_ski' => 'XC skiing',
        'backcountry_ski' => 'Backcountry skiing',
        'freediving' => 'Freediving',
        'fishing' => 'Fishing',
        'hiking' => 'Hiking',
        'packrafting' => 'Packrafting',
        _ => kind,
      };
}

class _ScoreCircle extends StatelessWidget {
  final int? score;
  final Color tintColor;
  const _ScoreCircle({required this.score, required this.tintColor});

  @override
  Widget build(BuildContext context) {
    final color = score == null
        ? Colors.grey.shade500
        : score! >= 70
            ? Colors.green.shade700
            : score! >= 40
                ? Colors.amber.shade700
                : Colors.red.shade600;
    return Container(
      width: 48,
      height: 48,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: color.withValues(alpha: 0.12),
        border: Border.all(color: color, width: 2),
      ),
      alignment: Alignment.center,
      child: Text(
        score?.toString() ?? '—',
        style: TextStyle(color: color, fontSize: 18, fontWeight: FontWeight.w700),
      ),
    );
  }
}

class _Chip extends StatelessWidget {
  final IconData icon;
  final String label;
  final Color tint;
  const _Chip({required this.icon, required this.label, required this.tint});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: tint.withValues(alpha: 0.10),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: tint.withValues(alpha: 0.4)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 12, color: tint),
          const SizedBox(width: 4),
          Text(label,
              style: TextStyle(color: tint, fontWeight: FontWeight.w500, fontSize: 12)),
        ],
      ),
    );
  }
}

class _WarningSnippet extends StatelessWidget {
  final AnalysisWarning warning;
  const _WarningSnippet({required this.warning});

  @override
  Widget build(BuildContext context) {
    final color = switch (warning.severity) {
      WarningSeverity.danger => Colors.red.shade700,
      WarningSeverity.caution => Colors.orange.shade700,
      WarningSeverity.info => Colors.blue.shade700,
    };
    return Row(
      children: [
        Icon(Icons.warning_amber_rounded, size: 14, color: color),
        const SizedBox(width: 4),
        Expanded(
          child: Text(
            warning.title,
            style: TextStyle(color: color, fontSize: 12, fontWeight: FontWeight.w600),
            overflow: TextOverflow.ellipsis,
          ),
        ),
      ],
    );
  }
}
