import 'package:flutter/material.dart';

class MapControls extends StatelessWidget {
  final List<Widget> controls;

  const MapControls({
    super.key,
    required this.controls,
  });

  @override
  Widget build(BuildContext context) {
    return Positioned(
      top: 80,
      right: 16,
      child: Column(
        children: controls,
      ),
    );
  }
}