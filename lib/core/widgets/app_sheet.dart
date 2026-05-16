import 'package:flutter/material.dart';
import 'package:turbo/core/theme/tokens.dart';

/// Standard bottom sheet shell: rounded top corners, drag handle, a title
/// row with close button, and 16dp body padding.
///
/// Opt-outs: `LayerSelectionSheet` (edge-to-edge horizontal carousel) and
/// `DownloadDetailsSheet` (currently keeps its own padding) continue to use
/// `showModalBottomSheet` directly.
Future<T?> showAppSheet<T>(
  BuildContext context, {
  required String title,
  required WidgetBuilder builder,
  bool isScrollControlled = true,
  bool enableDrag = true,
  bool showCloseButton = true,
  bool showDragHandle = true,
}) {
  return showModalBottomSheet<T>(
    context: context,
    isScrollControlled: isScrollControlled,
    useSafeArea: true,
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
    final colorScheme = Theme.of(context).colorScheme;
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
          if (showDragHandle)
            Center(
              child: Container(
                width: 32,
                height: 4,
                margin: const EdgeInsets.only(bottom: AppSpacing.s),
                decoration: BoxDecoration(
                  color: colorScheme.outlineVariant,
                  // The drag-handle pill is the one place in the codebase
                  // that uses a 2dp radius; intentionally localized here so
                  // no callsite needs Radius.circular(2).
                  borderRadius: const BorderRadius.all(Radius.circular(2)),
                ),
              ),
            ),
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
