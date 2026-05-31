import 'package:flutter/material.dart';

/// Centered "Loading…" / "Could not load…" placeholder used by every
/// per-kind detail sheet for the *outer* activity-record gate (without
/// the activity we can't draw the chassis at all). Deliberately
/// static — no spinner, no shimmer — because the whole rewrite was
/// motivated by the broken infinite-shimmer UX.
class ActivityLoadingHint extends StatelessWidget {
  final String message;
  final String? subline;
  final IconData icon;

  const ActivityLoadingHint({
    super.key,
    required this.message,
    this.subline,
    this.icon = Icons.hourglass_empty,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 48, horizontal: 24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 32, color: theme.colorScheme.outline),
          const SizedBox(height: 12),
          Text(message, style: theme.textTheme.titleMedium),
          if (subline != null) ...[
            const SizedBox(height: 4),
            Text(
              subline!,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ],
      ),
    );
  }
}
