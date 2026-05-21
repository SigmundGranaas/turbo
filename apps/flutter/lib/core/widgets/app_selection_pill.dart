import 'package:flutter/material.dart';
import 'package:turbo/app/tokens.dart';

/// Fully rounded selection pill used wherever the app shows "one of N"
/// choices. Selection is signaled by background color only — no checkmark.
///
/// Use everywhere we previously reached for `ChoiceChip` or a hand-rolled
/// `_DayChip`/`_PresetTabs` clone: the weather sheet tab bar, the day
/// strip, and the pin sheet's Info/Weather toggle.
class AppSelectionPill extends StatelessWidget {
  final Widget child;
  final bool selected;
  final VoidCallback? onTap;
  final IconData? leadingIcon;
  final EdgeInsetsGeometry padding;

  const AppSelectionPill({
    super.key,
    required this.child,
    required this.selected,
    this.onTap,
    this.leadingIcon,
    this.padding = const EdgeInsets.symmetric(
      horizontal: AppSpacing.l,
      vertical: AppSpacing.s,
    ),
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final bg = selected
        ? colorScheme.primary
        : colorScheme.surfaceContainerHigh;
    final fg = selected ? colorScheme.onPrimary : colorScheme.onSurface;
    final radius = BorderRadius.circular(AppRadius.pill);

    return Material(
      color: bg,
      borderRadius: radius,
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: padding,
          child: DefaultTextStyle.merge(
            style: TextStyle(color: fg),
            child: IconTheme.merge(
              data: IconThemeData(color: fg, size: 18),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  if (leadingIcon != null) ...[
                    Icon(leadingIcon),
                    const SizedBox(width: AppSpacing.xs),
                  ],
                  child,
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
