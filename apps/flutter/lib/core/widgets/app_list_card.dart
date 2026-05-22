import 'package:flutter/material.dart';
import 'package:turbo/app/tokens.dart';

/// Tappable list-style row card used inside bottom sheets.
///
/// Replaces three identical `_FormatCard` definitions that previously lived
/// inside the location-icon picker sheet and the two export sheets. Uses the
/// theme's `cardTheme` (elevation 0, `surfaceContainerLow`, radius 16) so it
/// matches the grouped controls in settings — one card style for the whole
/// app.
///
/// Usage shapes:
///   • Tap-the-whole-row drill-down: pass [onTap], no [trailing].
///   • Per-row action affordances (e.g. share / save): pass [trailing]
///     widgets and leave [onTap] null.
class AppListCard extends StatelessWidget {
  final IconData icon;
  final String title;
  final String? subtitle;
  final List<Widget> trailing;
  final VoidCallback? onTap;

  const AppListCard({
    super.key,
    required this.icon,
    required this.title,
    this.subtitle,
    this.trailing = const [],
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    Widget content = Padding(
      padding: const EdgeInsets.all(AppSpacing.m),
      child: Row(
        children: [
          Icon(icon, size: 28, color: colorScheme.primary),
          const SizedBox(width: AppSpacing.m),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(title, style: textTheme.titleSmall),
                if (subtitle != null) ...[
                  const SizedBox(height: 2),
                  Text(
                    subtitle!,
                    style: textTheme.bodySmall?.copyWith(
                      color: colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ],
            ),
          ),
          ...trailing,
        ],
      ),
    );

    if (onTap != null) {
      content = InkWell(
        borderRadius: BorderRadius.circular(AppRadius.l),
        onTap: onTap,
        child: content,
      );
    }

    return Card(
      clipBehavior: Clip.antiAlias,
      child: content,
    );
  }
}
