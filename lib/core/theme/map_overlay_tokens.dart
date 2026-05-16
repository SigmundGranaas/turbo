import 'package:flutter/material.dart';

/// Colors for editor overlays drawn directly on the map (selection polygons,
/// draggable handles). Distinct from `LocationMarkerTokens` — these are
/// editor chrome, not user location.
class MapOverlayTokens {
  /// Translucent fill for an in-progress selection polygon.
  static const Color selectionFill = Color(0x331565C0); // blue 800 @ 0.2
  /// Solid border for the selection polygon.
  static const Color selectionBorder = Color(0xFF1565C0);
  /// Border around a draggable rectangle handle on the map.
  static const Color handleBorder = Color(0xFFFFFFFF);
  /// Drop shadow under a draggable rectangle handle.
  static const Color handleShadow = Color(0x4D000000);
}
