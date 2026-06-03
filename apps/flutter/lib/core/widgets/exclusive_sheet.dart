import 'package:flutter/material.dart';

/// Whether a coordinated *top-level* modal sheet is currently open.
bool _topSheetOpen = false;

/// The one way to present a bottom sheet. Every feature routes through here (or
/// [showAppSheet], which delegates to it) so there is a single, predictable
/// sheet mechanism instead of dozens of ad-hoc `showModalBottomSheet` calls
/// with drifting params.
///
/// Top-level sheets ([replace] = true, the default) are single-at-a-time: if a
/// coordinated top-level sheet is already open it's dismissed first, so tapping
/// around the map never piles sheets on top of one another. A sheet opened
/// *from within* another sheet (an overflow menu, a sub-picker, an
/// edit/export/add-to-collection step) passes `replace: false` so it stacks on
/// its parent instead of closing it.
///
/// See `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 3).
Future<T?> showExclusiveSheet<T>(
  BuildContext context, {
  required WidgetBuilder builder,
  bool isScrollControlled = true,
  bool useSafeArea = true,
  bool replace = true,
  bool enableDrag = true,
  bool isDismissible = true,
  Color? backgroundColor,
  BoxConstraints? constraints,
  ShapeBorder? shape,
}) async {
  if (replace && _topSheetOpen) {
    // Drop the top-level sheet currently on top before presenting the new one.
    Navigator.of(context).maybePop();
  }
  // Only top-level sheets own the single-active flag; nested sheets stack and
  // must not flip it (else closing a nested sheet would mark the parent gone).
  if (replace) _topSheetOpen = true;
  try {
    return await showModalBottomSheet<T>(
      context: context,
      isScrollControlled: isScrollControlled,
      useSafeArea: useSafeArea,
      enableDrag: enableDrag,
      isDismissible: isDismissible,
      backgroundColor: backgroundColor,
      constraints: constraints,
      shape: shape,
      builder: builder,
    );
  } finally {
    if (replace) _topSheetOpen = false;
  }
}
