import 'package:flutter/material.dart';

/// In-form grouping container. Flat (elevation 0), `surfaceContainerLow`,
/// radius 16 (inherited from `cardTheme`).
///
/// Used to wrap one or more related controls (a `SwitchListTile`, a slider
/// row, a color picker) on a settings-style page.
class AppGroupedCard extends StatelessWidget {
  final Widget child;
  final EdgeInsetsGeometry padding;

  const AppGroupedCard({
    super.key,
    required this.child,
    this.padding = EdgeInsets.zero,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(padding: padding, child: child),
    );
  }
}
