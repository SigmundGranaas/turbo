import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_dialog.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';

import '../data/location_repository.dart';
import '../data/marker_export_service.dart';
import '../data/marker_selection_provider.dart';
import '../data/viewport_marker_provider.dart';
import '../models/marker.dart';

final _log = Logger('MarkerSelectionBar');

/// Floating bottom action bar that appears while [markerSelectionProvider]
/// has any entries. Provides bulk delete + export.
class MarkerSelectionBar extends ConsumerWidget {
  const MarkerSelectionBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final selection = ref.watch(markerSelectionProvider);
    if (selection.isEmpty) return const SizedBox.shrink();

    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;

    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(
            AppSpacing.l, 0, AppSpacing.l, AppSpacing.l),
        child: Material(
          elevation: 4,
          borderRadius: BorderRadius.circular(AppRadius.l),
          color: colorScheme.surfaceContainerHigh,
          child: Padding(
            padding: const EdgeInsets.symmetric(
                horizontal: AppSpacing.m, vertical: AppSpacing.s),
            child: Row(
              children: [
                IconButton(
                  icon: const Icon(Icons.close),
                  tooltip: l10n.cancel,
                  onPressed: () =>
                      ref.read(markerSelectionProvider.notifier).clear(),
                ),
                Expanded(
                  child: Text(
                    l10n.markersSelected(selection.length),
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                IconButton(
                  icon: const Icon(Icons.ios_share_outlined),
                  tooltip: l10n.bulkExport,
                  onPressed: () => _bulkExport(context, ref),
                ),
                IconButton(
                  icon: Icon(Icons.delete_outline,
                      color: colorScheme.error),
                  tooltip: l10n.delete,
                  onPressed: () => _bulkDelete(context, ref),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _bulkDelete(BuildContext context, WidgetRef ref) async {
    final l10n = context.l10n;
    final selection = ref.read(markerSelectionProvider);
    if (selection.isEmpty) return;

    final confirmed = await AppDialog.destructive(
      context,
      title: l10n.confirmBulkDeleteTitle,
      content: l10n.confirmBulkDeleteMessage(selection.length),
      destructiveLabel: l10n.delete,
    );
    if (!confirmed || !context.mounted) return;

    final uuids = selection.toList();
    try {
      await ref
          .read(locationRepositoryProvider.notifier)
          .deleteMarkers(uuids);
      ref.read(markerSelectionProvider.notifier).clear();
      ref.read(viewportMarkerNotifierProvider.notifier).invalidateCache();
      if (context.mounted) {
        AppSnackbars.success(context, l10n.bulkDeleteSuccess);
      }
    } catch (e, st) {
      _log.warning('Bulk delete failed', e, st);
      if (context.mounted) {
        AppSnackbars.error(context, l10n.errorDeletingLocation(e.toString()));
      }
    }
  }

  Future<void> _bulkExport(BuildContext context, WidgetRef ref) async {
    final selection = ref.read(markerSelectionProvider);
    if (selection.isEmpty) return;

    // Pull the full marker objects from the viewport cache, falling back to
    // the local repository data if the cache hasn't materialized them yet.
    final viewportMarkers =
        ref.read(viewportMarkerNotifierProvider).asData?.value ?? const <Marker>[];
    final repoMarkers =
        ref.read(locationRepositoryProvider).asData?.value ?? const <Marker>[];
    final byUuid = <String, Marker>{
      for (final m in [...repoMarkers, ...viewportMarkers]) m.uuid: m,
    };
    final selected = <Marker>[
      for (final uuid in selection)
        if (byUuid[uuid] != null) byUuid[uuid]!,
    ];
    if (selected.isEmpty) return;

    try {
      await MarkerExportService().shareManyAsGeoJson(selected);
    } catch (e, st) {
      _log.warning('Bulk export failed', e, st);
      if (context.mounted) {
        AppSnackbars.error(context, e.toString());
      }
    }
  }
}
