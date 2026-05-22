import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';

enum PathLineStyle {
  solid,
  dotted,
  dashed,
  dashDot;

  String get key {
    switch (this) {
      case PathLineStyle.solid:
        return 'solid';
      case PathLineStyle.dotted:
        return 'dotted';
      case PathLineStyle.dashed:
        return 'dashed';
      case PathLineStyle.dashDot:
        return 'dash_dot';
    }
  }

  static PathLineStyle fromKey(String? key) {
    switch (key) {
      case 'solid':
        return PathLineStyle.solid;
      case 'dotted':
        return PathLineStyle.dotted;
      case 'dashed':
        return PathLineStyle.dashed;
      case 'dash_dot':
        return PathLineStyle.dashDot;
      default:
        return PathLineStyle.solid;
    }
  }

  StrokePattern toStrokePattern() {
    switch (this) {
      case PathLineStyle.solid:
        return const StrokePattern.solid();
      case PathLineStyle.dotted:
        return const StrokePattern.dotted(spacingFactor: 1.5);
      case PathLineStyle.dashed:
        return StrokePattern.dashed(segments: [12, 6]);
      case PathLineStyle.dashDot:
        return StrokePattern.dashed(segments: [12, 6, 2, 6]);
    }
  }
}

const List<Color> pathColorPalette = [
  Color(0xFF1976D2), // Blue
  Color(0xFFD32F2F), // Red
  Color(0xFF388E3C), // Green
  Color(0xFFF57C00), // Orange
  Color(0xFF7B1FA2), // Purple
  Color(0xFF00897B), // Teal
  Color(0xFFC2185B), // Pink
  Color(0xFF546E7A), // Blue Grey
  Color(0xFF5D4037), // Brown
  Color(0xFF9E9D24), // Lime
];

String colorToHex(Color c) {
  final r = (c.r * 255.0).round().clamp(0, 255);
  final g = (c.g * 255.0).round().clamp(0, 255);
  final b = (c.b * 255.0).round().clamp(0, 255);
  return ((r << 16) | (g << 8) | b).toRadixString(16).padLeft(6, '0').toUpperCase();
}

Color? hexToColor(String? hex) {
  if (hex == null || hex.length != 6) return null;
  final parsed = int.tryParse(hex, radix: 16);
  if (parsed == null) return null;
  return Color(0xFF000000 | parsed);
}
