import 'package:flutter/widgets.dart';

/// Coordinates transient, non-modal surfaces (currently the search dropdown,
/// which is an `OverlayEntry`, not a route) against the modal layer.
///
/// The search dropdown used to be able to linger *over* a bottom sheet — a
/// third, uncoordinated overlay system. Rather than wiring every one of the
/// ~30 sheet call sites, [SearchDismissObserver] dismisses whatever transient
/// is registered here whenever ANY route is pushed (a sheet, dialog, or page).
/// One mechanism, all call sites covered. See
/// `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 3,
/// state-combining pass).
class TransientSurface {
  TransientSurface._();

  static VoidCallback? _searchDismiss;

  /// The active search dropdown registers its remover here while visible.
  static void setSearchDismisser(VoidCallback dismiss) =>
      _searchDismiss = dismiss;

  /// Clears the registration (only if it's still the same callback, so a
  /// stale clear can't wipe a newer registration).
  static void clearSearchDismisser(VoidCallback dismiss) {
    if (identical(_searchDismiss, dismiss)) _searchDismiss = null;
  }

  /// Dismiss the active transient, if any. Cleared before invoking so the
  /// dismisser's own cleanup (which calls [clearSearchDismisser]) is a no-op
  /// rather than recursing.
  static void dismiss() {
    final cb = _searchDismiss;
    _searchDismiss = null;
    cb?.call();
  }
}

/// Closes the active search dropdown whenever a route is pushed (modal sheet,
/// dialog, or page), so transient overlays never sit on top of a sheet.
/// Registered on the root [MaterialApp].
class SearchDismissObserver extends NavigatorObserver {
  @override
  void didPush(Route<dynamic> route, Route<dynamic>? previousRoute) {
    TransientSurface.dismiss();
    super.didPush(route, previousRoute);
  }
}
