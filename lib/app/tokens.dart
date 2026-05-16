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
