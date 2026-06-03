import 'package:flutter/material.dart';

/// Makes a floating bottom tool panel (route planning, measuring, …)
/// dismissible by **swiping it down**, matching the drag-to-dismiss of the
/// app's modal sheets. Purely a gesture wrapper — the panel supplies its own
/// chrome and an integrated [SheetDragHandle] at the top, so there's no
/// separate floating affordance.
class DismissibleToolSheet extends StatelessWidget {
  final Widget child;
  final VoidCallback onDismiss;

  /// Downward fling velocity (px/s) past which a swipe counts as a dismiss.
  static const double _dismissVelocity = 250;

  const DismissibleToolSheet({
    super.key,
    required this.child,
    required this.onDismiss,
  });

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.deferToChild,
      onVerticalDragEnd: (details) {
        if ((details.primaryVelocity ?? 0) > _dismissVelocity) onDismiss();
      },
      child: child,
    );
  }
}
