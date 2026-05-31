import 'package:flutter/material.dart';

import '../../models/activity_analysis.dart';

/// Horizontal strip of suggested time windows. The first chip auto-renders
/// as a richer "go now / go later" CTA — the user's primary decision is
/// "when," so put the answer up front.
class SuggestedWindowsStrip extends StatelessWidget {
  final List<AnalysisTimeWindow> windows;
  final Color tintColor;

  const SuggestedWindowsStrip({
    super.key,
    required this.windows,
    required this.tintColor,
  });

  @override
  Widget build(BuildContext context) {
    if (windows.isEmpty) return const SizedBox.shrink();
    final first = windows.first;
    final rest = windows.skip(1).toList(growable: false);

    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 4, 16, 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _PrimaryWindow(window: first, tintColor: tintColor),
          if (rest.isNotEmpty) ...[
            const SizedBox(height: 8),
            SizedBox(
              height: 44,
              child: ListView.separated(
                scrollDirection: Axis.horizontal,
                itemCount: rest.length,
                separatorBuilder: (_, _) => const SizedBox(width: 8),
                itemBuilder: (_, i) => _SecondaryChip(window: rest[i]),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _PrimaryWindow extends StatelessWidget {
  final AnalysisTimeWindow window;
  final Color tintColor;
  const _PrimaryWindow({required this.window, required this.tintColor});

  @override
  Widget build(BuildContext context) {
    final color = _qualityColor(window.quality, tintColor);
    final cta = _ctaLabel(window);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.10),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: color.withValues(alpha: 0.5)),
      ),
      child: Row(
        children: [
          Icon(_iconFor(window.quality), color: color),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  cta,
                  style: TextStyle(
                    color: color,
                    fontSize: 15,
                    fontWeight: FontWeight.w700,
                  ),
                ),
                if (window.reason != null) ...[
                  const SizedBox(height: 2),
                  Text(
                    window.reason!,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _SecondaryChip extends StatelessWidget {
  final AnalysisTimeWindow window;
  const _SecondaryChip({required this.window});

  @override
  Widget build(BuildContext context) {
    final color = _qualityColor(window.quality, Colors.grey.shade700);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.10),
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(_iconFor(window.quality), color: color, size: 14),
          const SizedBox(width: 6),
          Text(
            window.label,
            style: TextStyle(color: color, fontWeight: FontWeight.w600),
          ),
        ],
      ),
    );
  }
}

String _ctaLabel(AnalysisTimeWindow w) {
  final now = DateTime.now();
  final secondsToStart = w.start.difference(now).inSeconds;
  if (secondsToStart <= 0 && now.isBefore(w.end)) return 'Go now — ${w.label}';
  if (secondsToStart > 0) return 'Go later: ${w.label}';
  return w.label;
}

Color _qualityColor(WindowQuality q, Color fallback) => switch (q) {
      WindowQuality.excellent => Colors.green.shade700,
      WindowQuality.good => Colors.green.shade600,
      WindowQuality.marginal => Colors.orange.shade700,
      WindowQuality.avoid => Colors.red.shade700,
    };

IconData _iconFor(WindowQuality q) => switch (q) {
      WindowQuality.excellent => Icons.bolt_rounded,
      WindowQuality.good => Icons.check_rounded,
      WindowQuality.marginal => Icons.access_time_rounded,
      WindowQuality.avoid => Icons.do_not_disturb_alt_rounded,
    };
