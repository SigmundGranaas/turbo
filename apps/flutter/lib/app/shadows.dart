import 'package:flutter/material.dart';

/// Shadows used in *map overlay* contexts where Material's ambient elevation
/// reads wrong against tile imagery (e.g. measurement point markers, viewport
/// marker labels). For in-app surfaces, prefer Material elevation instead.
class AppShadows {
  static const List<BoxShadow> mapOverlay = [
    BoxShadow(
      color: Color(0x40000000),
      blurRadius: 4,
      offset: Offset(0, 2),
    ),
  ];
}
