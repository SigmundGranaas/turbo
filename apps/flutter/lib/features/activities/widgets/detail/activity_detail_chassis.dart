import 'package:flutter/material.dart';

import 'activity_chip_row.dart' show ActivityModuleCard;

/// One action in the bottom action row of every activity detail
/// screen. Same vertical icon + uppercase label vocabulary the design
/// uses (EDIT, EXPORT, DELETE, LOG VISIT…).
class ActivityAction {
  final IconData icon;
  final String label;
  final VoidCallback? onTap;
  final Color? color;
  const ActivityAction({
    required this.icon,
    required this.label,
    this.onTap,
    this.color,
  });

  /// The standard Log-visit / Edit / Delete action triple every
  /// per-kind sheet has. Centralised here so the icons and labels
  /// stay consistent and we don't ship six copies of the same trio.
  /// Pass `null` for any of the callbacks to omit that action.
  static List<ActivityAction> standardTriple(
    BuildContext context, {
    VoidCallback? onLogVisit,
    VoidCallback? onEdit,
    VoidCallback? onDelete,
  }) {
    final errorColor = Theme.of(context).colorScheme.error;
    return [
      if (onLogVisit != null)
        ActivityAction(
          icon: Icons.note_add_outlined,
          label: 'Log visit',
          onTap: onLogVisit,
        ),
      if (onEdit != null)
        ActivityAction(
          icon: Icons.edit_outlined,
          label: 'Edit',
          onTap: onEdit,
        ),
      if (onDelete != null)
        ActivityAction(
          icon: Icons.delete_outline,
          label: 'Delete',
          color: errorColor,
          onTap: onDelete,
        ),
    ];
  }
}

/// The shared detail-screen chassis. Every activity kind uses this
/// shell so the order, padding, and type rhythm stay identical across
/// the six kinds. Per-kind specifics ride in the slot widgets the
/// caller passes — verdict, map, weather, stats, module, description,
/// actions.
///
/// **Renders progressively.** The chassis itself has no async
/// dependency. The title row, stats, description, and actions paint
/// instantly from the typed activity record. The verdict, map, and
/// weather slots accept a [Widget?] — if the orchestrator is still
/// fetching the analysis, callers pass a quiet placeholder rather than
/// blocking the entire screen on a spinner. This is the architectural
/// fix for the "spins forever" complaint: static activity content is
/// never gated on the conditions call.
///
/// When [onRefresh] is set the chassis wraps its scroll view in a
/// [RefreshIndicator] — sheets pass `ref.invalidate(analysisProvider)`
/// here so pull-down (and the weather panel's refresh action) becomes
/// a real retry.
class ActivityDetailChassis extends StatelessWidget {
  /// Kind tint applied to the title-row icon ring and the weather
  /// accent. Falls through to the action row's color for the default
  /// actions unless an [ActivityAction.color] is set.
  final Color tintColor;

  /// Material icon shown inside the 44px tinted icon ring.
  final IconData icon;

  final String title;
  final String? place;

  /// Optional verdict card. When null, the slot is omitted entirely —
  /// preferred over passing a spinner when the orchestrator is still
  /// fetching.
  final Widget? verdict;

  /// Map preview slot. Same — null collapses the slot, so the screen
  /// stays clean when geometry isn't ready.
  final Widget? mapPreview;

  /// Weather panel slot. Always show one (loading/error states are
  /// the panel's own concern) — but the caller can pass null to omit
  /// entirely for kinds that don't surface weather.
  final Widget? weather;

  /// Stat strip — title-cased above the value, identical type rhythm
  /// across kinds.
  final Widget? stats;

  /// Optional kind-specific module (predicted wax, aspect glyph,
  /// depth bar, …). Typed to [ActivityModuleCard] so the contract is
  /// enforced at compile time.
  final ActivityModuleCard? module;

  /// Free-form description text from the activity record.
  final String? description;

  /// Standard action row at the bottom. Empty list hides the row.
  final List<ActivityAction> actions;

  final VoidCallback? onClose;

  /// Whether the chassis should subtract the top safe-area inset. The
  /// chassis is used both as the content of a bottom sheet (top inset
  /// already handled by the sheet, default `false`) and as the root
  /// of a full-screen route (must respect the system status bar,
  /// `true`).
  final bool safeAreaTop;

  /// When non-null, wraps the scroll view in a [RefreshIndicator]
  /// whose [RefreshIndicator.onRefresh] calls this. Sheets pass
  /// `() async => ref.invalidate(analysisProvider)` so pull-down
  /// becomes a real retry.
  final Future<void> Function()? onRefresh;

  const ActivityDetailChassis({
    super.key,
    required this.tintColor,
    required this.icon,
    required this.title,
    this.place,
    this.verdict,
    this.mapPreview,
    this.weather,
    this.stats,
    this.module,
    this.description,
    this.actions = const [],
    this.onClose,
    this.safeAreaTop = false,
    this.onRefresh,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scroll = SingleChildScrollView(
      // RefreshIndicator needs the scrollable to be always-scrollable
      // so pull-down works even when content is short.
      physics: onRefresh == null
          ? null
          : const AlwaysScrollableScrollPhysics(),
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _TitleRow(
            tintColor: tintColor,
            icon: icon,
            title: title,
            place: place,
            onClose: onClose,
          ),
          ?verdict,
          ?mapPreview,
          ?weather,
          ?stats,
          ?module,
          if (description != null && description!.isNotEmpty) ...[
            const SizedBox(height: 14),
            Text(
              description!,
              style: TextStyle(
                fontSize: 14,
                height: 22 / 14,
                color: theme.colorScheme.onSurface,
              ),
            ),
          ],
          if (actions.isNotEmpty) ...[
            const SizedBox(height: 16),
            _ActionRow(actions: actions),
          ],
        ],
      ),
    );
    final refreshable = onRefresh == null
        ? scroll
        : RefreshIndicator(
            onRefresh: onRefresh!,
            color: tintColor,
            child: scroll,
          );
    return SafeArea(
      top: safeAreaTop,
      child: refreshable,
    );
  }
}

class _TitleRow extends StatelessWidget {
  final Color tintColor;
  final IconData icon;
  final String title;
  final String? place;
  final VoidCallback? onClose;
  const _TitleRow({
    required this.tintColor,
    required this.icon,
    required this.title,
    this.place,
    this.onClose,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Container(
            width: 44,
            height: 44,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: tintColor.withValues(alpha: 0.14),
              shape: BoxShape.circle,
            ),
            child: Icon(icon, color: tintColor, size: 22),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  title,
                  style: TextStyle(
                    fontSize: 22,
                    height: 28 / 22,
                    color: theme.colorScheme.onSurface,
                  ),
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                ),
                if (place != null && place!.isNotEmpty)
                  Text(
                    place!,
                    style: TextStyle(
                      fontSize: 13,
                      height: 18 / 13,
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
              ],
            ),
          ),
          if (onClose != null)
            IconButton(
              onPressed: onClose,
              icon: Icon(
                Icons.close,
                size: 20,
                color: theme.colorScheme.onSurfaceVariant,
              ),
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints(minWidth: 40, minHeight: 40),
            ),
        ],
      ),
    );
  }
}

class _ActionRow extends StatelessWidget {
  final List<ActivityAction> actions;
  const _ActionRow({required this.actions});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceEvenly,
      children: [
        for (final a in actions)
          InkResponse(
            onTap: a.onTap,
            radius: 32,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    a.icon,
                    size: 22,
                    color: a.color ?? theme.colorScheme.primary,
                  ),
                  const SizedBox(height: 4),
                  Text(
                    a.label.toUpperCase(),
                    style: TextStyle(
                      fontSize: 11,
                      height: 14 / 11,
                      fontWeight: FontWeight.w500,
                      letterSpacing: 0.5,
                      color: a.color ?? theme.colorScheme.primary,
                    ),
                  ),
                ],
              ),
            ),
          ),
      ],
    );
  }
}
