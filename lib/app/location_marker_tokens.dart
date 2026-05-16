import 'package:flutter/material.dart';

/// Colors for the user's "current location" marker. Replaces the hardcoded
/// `Colors.lightBlue` / `Colors.white` defaults that used to live in
/// `current_location_layer.dart` and `settings_page.dart`.
class LocationMarkerTokens {
  /// Filled body of the default location dot (and its halo at 0.3 alpha).
  static const Color defaultFill = Color(0xFF4FC3F7); // light blue 300
  /// Outline ring around the location dot when no user override is set.
  static const Color defaultOutline = Color(0xFFFFFFFF);
}
