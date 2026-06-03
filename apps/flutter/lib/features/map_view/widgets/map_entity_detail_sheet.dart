import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/sheet_drag_handle.dart';

import '../models/map_entity_action.dart';
import '../models/map_selection.dart';
import 'map_entity_action_bar.dart';

/// The single detail sheet for any [MapSelection]. Renders the selection's rich
/// body (if any) above the shared [MapEntityActionBar], so a coordinate, a
/// point entity and a path all present through one consistent surface instead
/// of each feature hand-rolling its own sheet.
///
/// The sheet sizes to its content (the body scrolls only if it's taller than
/// the screen) so a compact action set never leaves a void above the bar.
class MapEntityDetailSheet extends ConsumerWidget {
  final MapSelection selection;
  const MapEntityDetailSheet({super.key, required this.selection});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final entity = MapEntityActionContext(
      ref: ref,
      context: context,
      title: selection.title,
      point: selection.point,
      path: selection.path,
      includeStandardActions: selection.includeStandardActions,
      extraActions: selection.extraActions,
      onSaveAsTrack: selection.onSaveAsTrack,
      onAddToCollection: selection.onAddToCollection,
      onEdit: selection.onEdit,
      onShare: selection.onShare,
      onExport: selection.onExport,
      onDelete: selection.onDelete,
      afterJourneyAction: () => Navigator.of(context).maybePop(),
    );

    final body = selection.bodyBuilder?.call(context) ??
        Padding(
          padding: const EdgeInsets.fromLTRB(
              AppSpacing.l, 0, AppSpacing.l, AppSpacing.s),
          child: Text(
            selection.title,
            style: Theme.of(context).textTheme.titleMedium,
          ),
        );

    return Material(
      color: Theme.of(context).colorScheme.surface,
      borderRadius:
          const BorderRadius.vertical(top: Radius.circular(AppRadius.xl)),
      clipBehavior: Clip.antiAlias,
      child: SafeArea(
        top: false,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            _Header(onClose: () => Navigator.of(context).maybePop()),
            // Body takes only the height it needs; scrolls if it would exceed
            // the modal's max height. No fixed fraction → no empty gap.
            Flexible(
              child: SingleChildScrollView(
                padding: EdgeInsets.zero,
                child: body,
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(
                  AppSpacing.s, AppSpacing.s, AppSpacing.s, AppSpacing.s),
              child: MapEntityActionBar(entity: entity),
            ),
          ],
        ),
      ),
    );
  }
}

/// Fixed-height top bar: a centred drag-handle pill with a close button pinned
/// to the right, both vertically aligned so neither clips the other.
class _Header extends StatelessWidget {
  final VoidCallback onClose;
  const _Header({required this.onClose});

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 44,
      child: Stack(
        children: [
          const Align(
            alignment: Alignment.topCenter,
            child: SheetDragHandle(),
          ),
          Align(
            alignment: Alignment.centerRight,
            child: Padding(
              padding: const EdgeInsets.only(right: AppSpacing.s),
              child: IconButton(
                icon: const Icon(Icons.close),
                tooltip: MaterialLocalizations.of(context).closeButtonLabel,
                onPressed: onClose,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
