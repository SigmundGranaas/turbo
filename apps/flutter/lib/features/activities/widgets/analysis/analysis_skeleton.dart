import 'package:flutter/material.dart';

/// Shimmer placeholder mirroring the analysis surface's layout: a tall
/// score block, a windows strip, and three driver cards. Used while the
/// orchestrator is fetching so the layout doesn't snap into place once
/// data arrives.
class AnalysisSkeleton extends StatefulWidget {
  const AnalysisSkeleton({super.key});

  @override
  State<AnalysisSkeleton> createState() => _AnalysisSkeletonState();
}

class _AnalysisSkeletonState extends State<AnalysisSkeleton>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1100),
    )..repeat(reverse: true);
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, _) {
        final base = Theme.of(context).colorScheme.surfaceContainerHighest;
        final highlight = Theme.of(context).colorScheme.surfaceContainerHigh;
        final color = Color.lerp(base, highlight, _controller.value)!;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _block(color, height: 96, margin: const EdgeInsets.fromLTRB(20, 16, 20, 12)),
            _block(color, height: 60, margin: const EdgeInsets.fromLTRB(16, 4, 16, 12)),
            for (var i = 0; i < 3; i++)
              _block(color, height: 96, margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 4)),
          ],
        );
      },
    );
  }

  static Widget _block(Color color, {required double height, required EdgeInsets margin}) =>
      Container(
        height: height,
        margin: margin,
        decoration: BoxDecoration(
          color: color,
          borderRadius: BorderRadius.circular(10),
        ),
      );
}
