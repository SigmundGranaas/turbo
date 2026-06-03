import 'package:flutter/material.dart';

/// The one palette for lines drawn on the map. The basemap is always the light
/// Kartverket topo, so these are fixed, theme-independent colours — NOT theme
/// `primary`/`error`, which flip to a pale "skin" salmon in dark mode and wash
/// out. Every route / followed-path / recording / navigation line uses these so
/// "a line on the map" reads the same way everywhere.
class MapLineStyle {
  MapLineStyle._();

  /// A route or followed path: dark slate, high contrast over terrain.
  static const Color path = Color(0xFF15233A);

  /// White casing stroke drawn *under* [path] so the line stays legible over
  /// busy contours. Render at the main stroke width + ~3.
  static final Color casing = Colors.white.withValues(alpha: 0.92);

  /// Live GPS recording trace — matches the recording halo on the location dot.
  static const Color recording = Color(0xFFD32F2F);

  /// Off-route / warning connectors.
  static const Color warning = Color(0xFFD32F2F);
}
