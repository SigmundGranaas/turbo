/// Design tokens. The canonical scale for the entire app.
///
/// Authors must use these instead of literal numbers in widget code. The
/// `allowed_border_radius` custom_lint rule enforces the radius set; the
/// other tokens are policed by review.
library;

class AppRadius {
  static const double s = 8.0;
  static const double m = 12.0;
  static const double l = 16.0;
  static const double xl = 28.0;
  static const double pill = 100.0;
}

class AppSpacing {
  static const double xs = 4.0;
  static const double s = 8.0;
  static const double m = 12.0;
  static const double l = 16.0;
  static const double xl = 24.0;
  static const double xxl = 32.0;
}

class AppElevation {
  static const double flat = 0.0;
  static const double raised = 3.0;
  static const double floating = 4.0;
}

class AppMotion {
  static const Duration fast = Duration(milliseconds: 150);
  static const Duration normal = Duration(milliseconds: 200);
  static const Duration slow = Duration(milliseconds: 300);
}

/// Stroke / fill tokens for the `external_vector_layers` overlays
/// (trails, MetAlerts areas, etc.). Keeps the per-source colours
/// theme-aware while pinning the geometry styling here instead of
/// scattering literals across `VectorDataLayer`.
class AppVectorOverlay {
  /// Stroke width for line features (trails). 3 px reads at z=10–14.
  static const double lineStrokeWidth = 3.0;

  /// Stroke width for polygon outlines (MetAlerts areas, protected
  /// areas). Thinner than lines because the fill provides emphasis.
  static const double polygonBorderStrokeWidth = 1.5;

  /// Alpha applied to the source colour for a polygon's fill — keeps
  /// the underlying map readable.
  static const double polygonFillAlpha = 0.18;

  /// Alpha applied to the source colour for line strokes / borders.
  static const double strokeAlpha = 0.8;
}
