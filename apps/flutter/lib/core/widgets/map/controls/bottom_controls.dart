import 'package:flutter/material.dart';

class BottomControls extends StatelessWidget {
  final Widget controls;

  const BottomControls({
    super.key,
    required this.controls,
  });

  @override
  Widget build(BuildContext context) {
    return Positioned(
      bottom: 20,
      left: 0,
      right: 0,
      child: Center(
        child: controls,
      ),
    );
  }
}