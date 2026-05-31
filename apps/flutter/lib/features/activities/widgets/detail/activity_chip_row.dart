import 'package:flutter/material.dart';

/// Wrapped pill row. Used for species lists, technique chips, etc.
/// Visual: surface fill, outlineVariant border, 12/16 medium label.
class ActivityChipRow extends StatelessWidget {
  final List<String> items;
  const ActivityChipRow({super.key, required this.items});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Wrap(
      spacing: 6,
      runSpacing: 6,
      children: [
        for (final t in items)
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
            decoration: BoxDecoration(
              color: theme.colorScheme.surface,
              border: Border.all(color: theme.colorScheme.outlineVariant),
              borderRadius: BorderRadius.circular(9999),
            ),
            child: Text(
              t,
              style: TextStyle(
                fontSize: 12,
                height: 16 / 12,
                fontWeight: FontWeight.w500,
                color: theme.colorScheme.onSurface,
              ),
            ),
          ),
      ],
    );
  }
}

/// Section card used by every per-kind module (predicted wax, aspect
/// glyph, depth bar, etc.). Same `surfaceContainer` fill and 14px
/// radius as the verdict/stats/weather cards, so kind modules read as
/// peers, not "extras".
class ActivityModuleCard extends StatelessWidget {
  final String? label;
  final Widget child;
  final EdgeInsetsGeometry padding;
  const ActivityModuleCard({
    super.key,
    this.label,
    required this.child,
    this.padding = const EdgeInsets.fromLTRB(14, 12, 14, 14),
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      margin: const EdgeInsets.only(top: 12),
      padding: padding,
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainer,
        borderRadius: BorderRadius.circular(14),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          if (label != null) ...[
            Text(
              label!.toUpperCase(),
              style: TextStyle(
                fontSize: 11,
                height: 14 / 11,
                fontWeight: FontWeight.w500,
                letterSpacing: 0.8,
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 8),
          ],
          child,
        ],
      ),
    );
  }
}
