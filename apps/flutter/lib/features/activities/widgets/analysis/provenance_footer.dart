import 'package:flutter/material.dart';

import '../../models/activity_analysis.dart';

/// Source chips at the bottom of the detail screen. Each chip surfaces
/// one provider's status (live / cached / stale / failed) so the user can
/// see at a glance whether the analysis they're acting on is built on
/// fresh signal or yesterday's. Tap-and-hold reveals the precise age.
class ProvenanceFooter extends StatelessWidget {
  final AnalysisProvenance provenance;
  const ProvenanceFooter({super.key, required this.provenance});

  @override
  Widget build(BuildContext context) {
    if (provenance.sources.isEmpty) return const SizedBox.shrink();
    final worstAge = _worstAgeSeconds(provenance.sources);
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (worstAge != null)
            Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: Text(
                'Oldest signal: ${_formatAge(worstAge)} ago',
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
              ),
            ),
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: [
              for (final hit in provenance.sources) _SourceChip(hit: hit),
            ],
          ),
        ],
      ),
    );
  }

  static int? _worstAgeSeconds(List<AnalysisSourceHit> sources) {
    int? worst;
    for (final h in sources) {
      final a = h.ageSeconds;
      if (a == null) continue;
      if (worst == null || a > worst) worst = a;
    }
    return worst;
  }

  static String _formatAge(int seconds) {
    if (seconds < 60) return '${seconds}s';
    final mins = seconds ~/ 60;
    if (mins < 60) return '${mins}m';
    final hours = mins ~/ 60;
    if (hours < 24) return '${hours}h';
    final days = hours ~/ 24;
    return '${days}d';
  }
}

class _SourceChip extends StatelessWidget {
  final AnalysisSourceHit hit;
  const _SourceChip({required this.hit});

  @override
  Widget build(BuildContext context) {
    final color = !hit.ok
        ? Colors.red.shade700
        : hit.fromCache
            ? Colors.grey.shade600
            : Colors.green.shade700;
    final label = '${hit.providerKey}${hit.fromCache ? ' (cached)' : ''}';
    final tooltip = hit.error ??
        (hit.ageSeconds != null
            ? '${hit.providerKey}: ${hit.ageSeconds}s old'
            : hit.providerKey);
    return Tooltip(
      message: tooltip,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.10),
          borderRadius: BorderRadius.circular(10),
          border: Border.all(color: color.withValues(alpha: 0.4)),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              hit.ok
                  ? (hit.fromCache ? Icons.cached : Icons.check_rounded)
                  : Icons.close_rounded,
              size: 12,
              color: color,
            ),
            const SizedBox(width: 4),
            Text(
              label,
              style: TextStyle(
                color: color,
                fontSize: 11,
                fontWeight: FontWeight.w500,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
