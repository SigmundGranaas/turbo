import 'package:flutter/material.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:turbo/core/widgets/sheet_drag_handle.dart';

/// Standard bottom sheet shell: rounded top corners, drag handle, a title
/// row with close button, and 16dp body padding. A thin convenience over
/// [showExclusiveSheet] — it supplies the shell chrome; the single sheet
/// mechanism still lives in one place.
Future<T?> showAppSheet<T>(
  BuildContext context, {
  required String title,
  required WidgetBuilder builder,
  bool isScrollControlled = true,
  bool enableDrag = true,
  bool showCloseButton = true,
  bool showDragHandle = true,
}) {
  return showExclusiveSheet<T>(
    context,
    isScrollControlled: isScrollControlled,
    enableDrag: enableDrag,
    builder: (sheetContext) => _AppSheetShell(
      title: title,
      showCloseButton: showCloseButton,
      showDragHandle: showDragHandle,
      child: Builder(builder: builder),
    ),
  );
}

class _AppSheetShell extends StatelessWidget {
  final String title;
  final Widget child;
  final bool showCloseButton;
  final bool showDragHandle;

  const _AppSheetShell({
    required this.title,
    required this.child,
    required this.showCloseButton,
    required this.showDragHandle,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final mediaQuery = MediaQuery.of(context);
    final bottomPadding = mediaQuery.viewInsets.bottom > 0
        ? mediaQuery.viewInsets.bottom
        : mediaQuery.padding.bottom;

    return Padding(
      padding: EdgeInsets.fromLTRB(
        AppSpacing.l,
        showDragHandle ? AppSpacing.m : AppSpacing.l,
        AppSpacing.l,
        AppSpacing.l + bottomPadding,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (showDragHandle) const SheetDragHandle(),
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Expanded(
                child: Text(title, style: textTheme.titleLarge),
              ),
              if (showCloseButton)
                IconButton(
                  onPressed: () => Navigator.pop(context),
                  icon: const Icon(Icons.close),
                ),
            ],
          ),
          const SizedBox(height: AppSpacing.l),
          Flexible(child: child),
        ],
      ),
    );
  }
}
