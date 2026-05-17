import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/widgets/action_button.dart';
import 'package:turbo/core/widgets/app_dialog.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/collections/api.dart';
import 'package:turbo/features/saved_paths/api.dart' show hexToColor;
import '../data/icon_service.dart';
import '../data/location_repository.dart';
import '../models/marker.dart';
import 'edit_location_sheet.dart';
import 'marker_export_options_sheet.dart';

enum MarkerInfoResult { updated, deleted }

class MarkerInfoSheet extends ConsumerStatefulWidget {
  final Marker marker;

  const MarkerInfoSheet({super.key, required this.marker});

  @override
  ConsumerState<MarkerInfoSheet> createState() => _MarkerInfoSheetState();
}

class _MarkerInfoSheetState extends ConsumerState<MarkerInfoSheet> {
  late Marker _marker;
  bool _isDeleting = false;

  @override
  void initState() {
    super.initState();
    _marker = widget.marker;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final mediaQuery = MediaQuery.of(context);
    final bottomPadding = mediaQuery.viewInsets.bottom > 0
        ? mediaQuery.viewInsets.bottom
        : mediaQuery.padding.bottom;
    final namedIcon = IconService().getIcon(context, _marker.icon);

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomPadding),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Header: icon + title + close
          Row(
            children: [
              Container(
                width: 48,
                height: 48,
                decoration: BoxDecoration(
                  color: colorScheme.primaryContainer,
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  namedIcon.icon,
                  color: colorScheme.onPrimaryContainer,
                  size: 24,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  _marker.title,
                  style: textTheme.titleLarge,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              IconButton(
                onPressed: () => Navigator.pop(context),
                icon: const Icon(Icons.close),
              ),
            ],
          ),
          const SizedBox(height: 16),

          // Details
          Row(
            children: [
              Icon(Icons.location_on_outlined,
                  size: 18, color: colorScheme.onSurfaceVariant),
              const SizedBox(width: 8),
              Text(
                '${_marker.position.latitude.toStringAsFixed(6)}, '
                '${_marker.position.longitude.toStringAsFixed(6)}',
                style: textTheme.bodyMedium?.copyWith(
                  color: colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),

          if (_marker.description != null &&
              _marker.description!.isNotEmpty) ...[
            const SizedBox(height: 8),
            Text(
              _marker.description!,
              style: textTheme.bodyMedium,
            ),
          ],

          const SizedBox(height: 12),
          _CollectionChipStrip(
            itemRef: CollectionItemRef(
              type: CollectionItemRef.typeMarker,
              uuid: _marker.uuid,
            ),
          ),
          const SizedBox(height: 16),

          // Actions row
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceEvenly,
            children: [
              ActionButton(
                icon: Icons.edit_outlined,
                label: l10n.edit,
                onTap: _openEdit,
              ),
              ActionButton(
                icon: Icons.folder_outlined,
                label: l10n.addToCollection,
                onTap: _openAddToCollection,
              ),
              ActionButton(
                icon: Icons.ios_share_outlined,
                label: l10n.export,
                onTap: _openExport,
              ),
              ActionButton(
                icon: Icons.delete_outline,
                label: l10n.delete,
                onTap: _isDeleting ? null : _confirmDelete,
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _openAddToCollection() async {
    await AddToCollectionSheet.show(
      context,
      CollectionItemRef(
        type: CollectionItemRef.typeMarker,
        uuid: _marker.uuid,
      ),
    );
  }

  Future<void> _openEdit() async {
    final result = await showModalBottomSheet<MarkerInfoResult>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (_) => EditLocationSheet(location: _marker),
    );
    if (result != null && mounted) {
      Navigator.of(context).pop(result);
    }
  }

  void _openExport() {
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (_) => MarkerExportOptionsSheet(marker: _marker),
    );
  }

  Future<void> _confirmDelete() async {
    final l10n = context.l10n;
    final confirmed = await AppDialog.destructive(
      context,
      title: l10n.confirmDeleteTitle,
      content: l10n.confirmDeleteMessage,
      destructiveLabel: l10n.delete,
    );
    if (confirmed) {
      await _deleteMarker();
    }
  }

  Future<void> _deleteMarker() async {
    final l10n = context.l10n;
    setState(() => _isDeleting = true);
    try {
      await ref
          .read(locationRepositoryProvider.notifier)
          .deleteMarker(_marker.uuid);
      await ref
          .read(collectionRepositoryProvider.notifier)
          .handleItemDeleted(CollectionItemRef(
            type: CollectionItemRef.typeMarker,
            uuid: _marker.uuid,
          ));
      if (mounted) {
        Navigator.of(context).pop(MarkerInfoResult.deleted);
      }
    } catch (error) {
      if (mounted) {
        AppSnackbars.error(context, l10n.errorDeletingLocation(error.toString()));
      }
    } finally {
      if (mounted) {
        setState(() => _isDeleting = false);
      }
    }
  }
}

class _CollectionChipStrip extends ConsumerWidget {
  final CollectionItemRef itemRef;

  const _CollectionChipStrip({required this.itemRef});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final asyncState = ref.watch(collectionRepositoryProvider);
    return asyncState.maybeWhen(
      data: (state) {
        final uuids = state.collectionsFor(itemRef);
        if (uuids.isEmpty) return const SizedBox.shrink();
        final byUuid = {for (final c in state.collections) c.uuid: c};
        final colorScheme = Theme.of(context).colorScheme;
        return SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Row(
            children: [
              for (final id in uuids)
                if (byUuid[id] != null)
                  Padding(
                    padding: const EdgeInsets.only(right: 6),
                    child: Chip(
                      avatar: Icon(
                        Icons.folder_outlined,
                        size: 16,
                        color: hexToColor(byUuid[id]!.colorHex) ??
                            colorScheme.primary,
                      ),
                      label: Text(byUuid[id]!.name),
                      visualDensity: VisualDensity.compact,
                    ),
                  ),
            ],
          ),
        );
      },
      orElse: () => const SizedBox.shrink(),
    );
  }
}

