import 'package:flutter/material.dart';

/// The four-stop condition palette from the design system's `COND`
/// vocabulary — green = clear-go, amber = pick your window, red =
/// don't, yellow = note-worthy but not a blocker. Kept as plain
/// constants instead of a [ThemeExtension] so const-constructible
/// widgets (verdict bar, aspect chips, score circle) can use them
/// directly. Update once here when the palette tunes; do not inline
/// these hex codes elsewhere.
abstract final class ConditionPalette {
  static const good = Color(0xFF388E3C);
  static const caution = Color(0xFFF57C00);
  static const stop = Color(0xFFD32F2F);
  static const warn = Color(0xFFFBC02D);
}
