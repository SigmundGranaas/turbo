import 'package:flutter/material.dart';

/// Lays out floating map overlays in deterministic, non-overlapping stacks
/// anchored to the top and bottom edges.
///
/// Replaces the hand-placed `Positioned` widgets in `MainMapPage` whose
/// hardcoded `bottom:` offsets (16 / 20 / 24) collided when more than one was
/// active. Children are stacked in a `Column`, so they can never overlap; each
/// overlay self-hides (returns `SizedBox.shrink`) when idle, so the column
/// collapses around whatever is actually showing. This is the composition seam
/// for map overlays — features hand the host a widget and a position; the host
/// owns collision-free placement. See
/// `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 3).
class MapOverlayHost extends StatelessWidget {
  /// Bottom-anchored overlays, listed top→bottom (the last entry sits nearest
  /// the bottom edge).
  final List<Widget> bottomChildren;

  /// Top-anchored overlays, listed top→bottom.
  final List<Widget> topChildren;

  /// Full-width bar flush to the bottom edge, beneath [bottomChildren]
  /// (e.g. a selection action bar). Optional.
  final Widget? bottomBar;

  const MapOverlayHost({
    super.key,
    this.bottomChildren = const [],
    this.topChildren = const [],
    this.bottomBar,
  });

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        if (bottomBar != null)
          Positioned(left: 0, right: 0, bottom: 0, child: bottomBar!),
        if (topChildren.isNotEmpty)
          Positioned(
            top: 16,
            left: 16,
            right: 16,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: _spaced(topChildren),
            ),
          ),
        if (bottomChildren.isNotEmpty)
          Positioned(
            bottom: 16,
            left: 16,
            right: 16,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: _spaced(bottomChildren),
            ),
          ),
      ],
    );
  }

  static List<Widget> _spaced(List<Widget> children) {
    final out = <Widget>[];
    for (var i = 0; i < children.length; i++) {
      if (i > 0) out.add(const SizedBox(height: 8));
      out.add(children[i]);
    }
    return out;
  }
}
