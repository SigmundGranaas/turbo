import 'package:flutter/material.dart';
import 'package:turbo/app/tokens.dart';

/// Centralised snackbar styling.
///
/// One consistent surface — the Material 3 *inverse* surface (high-contrast and
/// theme-correct in both light and dark; never the pale "skin" salmon of
/// `primaryContainer`/`errorContainer`) — with a leading status icon, a width
/// cap so it never spans a whole desktop window, and single-at-a-time
/// presentation (any current toast is cleared first) so they don't stack or
/// overlap.
///
/// Use sparingly: a snackbar should confirm an **async or otherwise invisible**
/// outcome — a track synced, a delete persisted, a background job finished, or
/// an error the user can't otherwise see — not echo an action whose result is
/// already plainly visible on screen.
class AppSnackbars {
  /// Below this width the toast floats near full-width (mobile); at or above it
  /// it's a fixed, centred pill so it doesn't stretch across a desktop window.
  static const double _desktopBreakpoint = 600;
  static const double _desktopWidth = 440;

  static void success(BuildContext context, String message) =>
      _show(context, message, icon: Icons.check_circle_outline);

  static void error(BuildContext context, String message) =>
      _show(context, message, icon: Icons.error_outline, isError: true);

  static void info(BuildContext context, String message) =>
      _show(context, message, icon: Icons.info_outline);

  static void _show(
    BuildContext context,
    String message, {
    required IconData icon,
    bool isError = false,
  }) {
    final scheme = Theme.of(context).colorScheme;
    final messenger = ScaffoldMessenger.of(context);
    final fg = scheme.onInverseSurface;
    // Errors tint the icon (and stay a touch longer); everything else uses the
    // neutral inverse foreground.
    final accent = isError ? scheme.error : fg;

    final width = MediaQuery.sizeOf(context).width;
    final fixedWidth = width >= _desktopBreakpoint ? _desktopWidth : null;

    final snackBar = SnackBar(
      content: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, color: accent, size: 20),
          const SizedBox(width: AppSpacing.s),
          Flexible(
            child: Text(
              message,
              style:
                  Theme.of(context).textTheme.bodyMedium?.copyWith(color: fg),
            ),
          ),
        ],
      ),
      backgroundColor: scheme.inverseSurface,
      behavior: SnackBarBehavior.floating,
      elevation: 6,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(AppRadius.l),
      ),
      width: fixedWidth,
      margin: fixedWidth == null ? const EdgeInsets.all(AppSpacing.l) : null,
      duration: Duration(seconds: isError ? 4 : 2),
    );

    messenger
      ..clearSnackBars()
      ..showSnackBar(snackBar);
  }
}
