import 'package:flutter/material.dart';

class MapControls extends StatelessWidget {
  final List<Widget> controls;
  final double? top;

  const MapControls({
    super.key,
    required this.controls,
    this.top,
  });

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;

    if (isMobile) {
      return Positioned(
        top: top ?? 72,
        right: 16,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: controls,
        ),
      );
    } else {
      return Positioned(
        bottom: 80,
        right: 16,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.end,
          children: controls,
        ),
      );
    }
  }
}
