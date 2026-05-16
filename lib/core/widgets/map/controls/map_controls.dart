import 'package:flutter/material.dart';
import 'package:turbo/core/theme/tokens.dart';

class MapControls extends StatelessWidget {
  final List<Widget> controls;
  final double? top;

  const MapControls({
    super.key,
    required this.controls,
    this.top,
  });

  /// Interleaves [AppSpacing.m] gaps between each control so the floating
  /// stack reads as separate buttons rather than a single rounded rail.
  List<Widget> _spaced(List<Widget> children) {
    if (children.length < 2) return children;
    final spaced = <Widget>[];
    for (var i = 0; i < children.length; i++) {
      spaced.add(children[i]);
      if (i != children.length - 1) {
        spaced.add(const SizedBox(height: AppSpacing.m));
      }
    }
    return spaced;
  }

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;

    if (isMobile) {
      return Positioned(
        top: top ?? 72,
        right: AppSpacing.l,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: _spaced(controls),
        ),
      );
    } else {
      return Positioned(
        bottom: 80,
        right: AppSpacing.l,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.end,
          children: _spaced(controls),
        ),
      );
    }
  }
}
