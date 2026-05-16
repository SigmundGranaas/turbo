import 'package:flutter/material.dart';
import 'package:turbo/app/tokens.dart';

/// Floating pill chrome for map-overlay chips (mode indicator, navigation
/// info). Intrinsic-width: the caller controls sizing via the child.
class AppPill extends StatelessWidget {
  final Widget child;
  final EdgeInsetsGeometry padding;

  const AppPill({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.symmetric(
      horizontal: AppSpacing.l,
      vertical: AppSpacing.m,
    ),
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: AppElevation.floating,
      color: Theme.of(context).colorScheme.surfaceContainer,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(AppRadius.xl),
      ),
      child: Padding(padding: padding, child: child),
    );
  }
}

/// Full-width version of [AppPill] for floating cards that hold multi-row
/// content (measuring controls, download progress toolbar). Optionally
/// constrained to `maxWidth` so it doesn't stretch on large screens.
class AppCardSurface extends StatelessWidget {
  final Widget child;
  final EdgeInsetsGeometry padding;
  final EdgeInsetsGeometry margin;
  final double? maxWidth;

  const AppCardSurface({
    super.key,
    required this.child,
    this.padding = EdgeInsets.zero,
    this.margin = EdgeInsets.zero,
    this.maxWidth,
  });

  @override
  Widget build(BuildContext context) {
    Widget card = Card(
      margin: margin,
      elevation: AppElevation.floating,
      color: Theme.of(context).colorScheme.surfaceContainer,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(AppRadius.xl),
      ),
      child: Padding(padding: padding, child: child),
    );
    if (maxWidth != null) {
      card = ConstrainedBox(
        constraints: BoxConstraints(maxWidth: maxWidth!),
        child: card,
      );
    }
    return card;
  }
}
