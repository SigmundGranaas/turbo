import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'action_button.dart';

/// A single entry in a [SheetActionBar].
///
/// Mark the rare destructive entry (delete, discard) with [isDestructive]
/// so the bar can tint it with the scheme's error colour both inline and
/// inside the overflow menu. A `null` [onPressed] renders the action
/// disabled rather than hiding it.
class SheetAction {
  final IconData icon;
  final String label;
  final VoidCallback? onPressed;
  final bool isDestructive;

  const SheetAction({
    required this.icon,
    required this.label,
    required this.onPressed,
    this.isDestructive = false,
  });
}

/// Adaptive action bar for detail / info bottom sheets.
///
/// Detail sheets across the app had each grown their own ad-hoc row or
/// `Wrap` of buttons, which overflowed on narrow phones once a feature
/// added a fifth or sixth action. This bar gives them one shared layout:
/// the first few actions render as equal-width icon-over-label buttons,
/// and any surplus collapses behind a single "More" button that opens a
/// secondary sheet listing the rest as list tiles.
///
/// The result is a sheet that can expose any number of actions without
/// crowding the row — the high-traffic actions stay one tap away while
/// the long tail moves one tap deeper.
class SheetActionBar extends StatelessWidget {
  final List<SheetAction> actions;

  /// Maximum number of buttons rendered in the row, *including* the
  /// "More" button when overflow is needed. Four reads comfortably down
  /// to ~320dp; lower it for sheets with longer labels.
  final int maxInline;

  const SheetActionBar({
    super.key,
    required this.actions,
    this.maxInline = 4,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;

    final List<SheetAction> inline;
    final List<SheetAction> overflow;
    if (actions.length <= maxInline) {
      inline = actions;
      overflow = const [];
    } else {
      // Reserve the last slot for the "More" affordance.
      inline = actions.take(maxInline - 1).toList();
      overflow = actions.skip(maxInline - 1).toList();
    }

    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        for (final action in inline)
          Expanded(
            child: ActionButton(
              icon: action.icon,
              label: action.label,
              onTap: action.onPressed,
              color: action.isDestructive ? colorScheme.error : null,
            ),
          ),
        if (overflow.isNotEmpty)
          Expanded(
            child: ActionButton(
              icon: Icons.more_horiz,
              label: context.l10n.more,
              onTap: () => _showOverflow(context, overflow),
            ),
          ),
      ],
    );
  }

  Future<void> _showOverflow(
      BuildContext context, List<SheetAction> overflow) {
    return showModalBottomSheet<void>(
      context: context,
      useSafeArea: true,
      builder: (sheetContext) {
        final colorScheme = Theme.of(sheetContext).colorScheme;
        return SafeArea(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const SizedBox(height: AppSpacing.s),
              for (final action in overflow)
                ListTile(
                  enabled: action.onPressed != null,
                  leading: Icon(
                    action.icon,
                    color: action.isDestructive ? colorScheme.error : null,
                  ),
                  title: Text(
                    action.label,
                    style: action.isDestructive
                        ? TextStyle(color: colorScheme.error)
                        : null,
                  ),
                  onTap: action.onPressed == null
                      ? null
                      : () {
                          // Dismiss the overflow menu first, then run the
                          // action against the originating sheet's context.
                          Navigator.of(sheetContext).pop();
                          action.onPressed!();
                        },
                ),
            ],
          ),
        );
      },
    );
  }
}
