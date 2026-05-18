import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:image_picker/image_picker.dart';
import 'package:turbo/core/widgets/action_button.dart';
import 'package:turbo/core/widgets/app_dialog.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/collections/api.dart';
import 'package:turbo/features/navigation/api.dart';
import 'package:turbo/features/saved_paths/api.dart' show hexToColor;
import 'package:turbo/features/weather/api.dart' show WeatherSummaryRow;
import '../data/icon_service.dart';
import '../data/location_repository.dart';
import '../data/marker_photo_repository.dart';
import '../models/marker.dart';
import '../models/marker_photo.dart';
import 'edit_location_sheet.dart';
import 'marker_export_options_sheet.dart';

enum MarkerInfoResult { updated, deleted, navigationStarted }

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
      child: SingleChildScrollView(
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
                tooltip: l10n.close,
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

          const SizedBox(height: 16),
          WeatherSummaryRow(
            position: _marker.position,
            title: _marker.title,
          ),
          _PhotoStrip(markerUuid: _marker.uuid),

          const SizedBox(height: 12),
          _CollectionChipStrip(
            itemRef: CollectionItemRef(
              type: CollectionItemRef.typeMarker,
              uuid: _marker.uuid,
            ),
            onTap: _openAddToCollection,
          ),
          const SizedBox(height: 16),

          // Actions row
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceEvenly,
            children: [
              ActionButton(
                icon: Icons.navigation_outlined,
                label: l10n.navigateToHere,
                onTap: _startNavigation,
              ),
              ActionButton(
                icon: Icons.edit_outlined,
                label: l10n.edit,
                onTap: _openEdit,
              ),
              if (!kIsWeb)
                ActionButton(
                  icon: Icons.add_a_photo_outlined,
                  label: 'Photo',
                  onTap: () => _addPhoto(context),
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
      ),
    );
  }

  Future<void> _addPhoto(BuildContext context) async {
    final messenger = ScaffoldMessenger.maybeOf(context);
    final choice = await showModalBottomSheet<ImageSource>(
      context: context,
      builder: (ctx) => SafeArea(
        child: Wrap(children: [
          ListTile(
            leading: const Icon(Icons.photo_library_outlined),
            title: const Text('Choose from gallery'),
            onTap: () => Navigator.of(ctx).pop(ImageSource.gallery),
          ),
          ListTile(
            leading: const Icon(Icons.photo_camera_outlined),
            title: const Text('Take a photo'),
            onTap: () => Navigator.of(ctx).pop(ImageSource.camera),
          ),
        ]),
      ),
    );
    if (choice == null) return;
    try {
      final xfile = await ImagePicker().pickImage(
        source: choice, maxWidth: 2048, imageQuality: 85);
      if (xfile == null) return;
      final svc = await ref.read(markerPhotoServiceProvider.future);
      await svc.addPhoto(markerUuid: _marker.uuid, source: File(xfile.path));
    } catch (e) {
      messenger?.showSnackBar(
          SnackBar(content: Text('Could not attach photo: $e')));
    }
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

  Future<void> _startNavigation() async {
    final l10n = context.l10n;
    final notifier = ref.read(navigationStateProvider.notifier);
    final currentState = ref.read(navigationStateProvider);

    // Same target → no-op; just tell the user.
    if (currentState.isActive && currentState.target == _marker.position) {
      AppSnackbars.info(context, l10n.alreadyNavigatingHere);
      return;
    }

    // Different target active → confirm before replacing.
    if (currentState.isActive && currentState.target != _marker.position) {
      final confirmed = await AppDialog.confirm(
        context,
        title: l10n.replaceNavigationTitle,
        content: l10n.replaceNavigationMessage,
        confirmLabel: l10n.replace,
      );
      if (!confirmed || !mounted) return;
    }

    notifier.startNavigation(_marker.position);
    if (mounted) {
      Navigator.of(context).pop(MarkerInfoResult.navigationStarted);
    }
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
  final VoidCallback onTap;

  const _CollectionChipStrip({required this.itemRef, required this.onTap});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    final asyncState = ref.watch(collectionRepositoryProvider);
    return asyncState.maybeWhen(
      data: (state) {
        final uuids = state.collectionsFor(itemRef);
        final byUuid = {for (final c in state.collections) c.uuid: c};
        return SizedBox(
          height: 36,
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: [
                if (uuids.isEmpty)
                  ActionChip(
                    avatar: Icon(
                      Icons.folder_outlined,
                      size: 16,
                      color: colorScheme.primary,
                    ),
                    label: Text(l10n.addToCollection),
                    onPressed: onTap,
                    shape: const StadiumBorder(),
                    side: BorderSide(color: colorScheme.outlineVariant),
                    visualDensity: VisualDensity.compact,
                  )
                else ...[
                  for (final id in uuids)
                    if (byUuid[id] != null)
                      Padding(
                        padding: const EdgeInsets.only(right: 6),
                        child: ActionChip(
                          avatar: Icon(
                            Icons.folder_outlined,
                            size: 16,
                            color: hexToColor(byUuid[id]!.colorHex) ??
                                colorScheme.primary,
                          ),
                          label: Text(byUuid[id]!.name),
                          onPressed: onTap,
                          shape: const StadiumBorder(),
                          side: BorderSide(color: colorScheme.outlineVariant),
                          visualDensity: VisualDensity.compact,
                        ),
                      ),
                  IconButton(
                    tooltip: l10n.addToCollection,
                    iconSize: 20,
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(Icons.add_circle_outline),
                    onPressed: onTap,
                  ),
                ],
              ],
            ),
          ),
        );
      },
      orElse: () => const SizedBox.shrink(),
    );
  }
}

/// Horizontal thumbnail strip + "add photo" trailing button. Tapping a
/// thumbnail opens a fullscreen viewer with a delete action.
class _PhotoStrip extends ConsumerWidget {
  final String markerUuid;
  const _PhotoStrip({required this.markerUuid});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (kIsWeb) return const SizedBox.shrink();
    final theme = Theme.of(context);
    final async = ref.watch(markerPhotosProvider(markerUuid));

    return async.when(
      loading: () => const SizedBox.shrink(),
      error: (e, _) => Text('Photo error: $e', style: theme.textTheme.bodySmall),
      data: (photos) {
        if (photos.isEmpty) return const SizedBox.shrink();
        return Padding(
          padding: const EdgeInsets.only(top: 12),
          child: SizedBox(
            height: 72,
            child: ListView.separated(
              scrollDirection: Axis.horizontal,
              itemCount: photos.length,
              separatorBuilder: (_, _) => const SizedBox(width: 8),
              itemBuilder: (context, i) => _PhotoThumb(photo: photos[i]),
            ),
          ),
        );
      },
    );
  }
}

class _PhotoThumb extends ConsumerWidget {
  final MarkerPhoto photo;
  const _PhotoThumb({required this.photo});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return GestureDetector(
      onTap: () => _openViewer(context, ref),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(8),
        child: Image.file(
          File(photo.filePath),
          width: 80,
          height: 80,
          fit: BoxFit.cover,
          errorBuilder: (_, _, _) => Container(
            width: 80,
            height: 80,
            color: Theme.of(context).colorScheme.surfaceContainerHighest,
            child: const Icon(Icons.broken_image_outlined),
          ),
        ),
      ),
    );
  }

  Future<void> _openViewer(BuildContext context, WidgetRef ref) async {
    final result = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        fullscreenDialog: true,
        builder: (_) => _PhotoViewer(photo: photo),
      ),
    );
    if (result == true) {
      final svc = await ref.read(markerPhotoServiceProvider.future);
      await svc.removePhoto(photo.uuid);
    }
  }
}

class _PhotoViewer extends StatelessWidget {
  final MarkerPhoto photo;
  const _PhotoViewer({required this.photo});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.black,
      appBar: AppBar(
        backgroundColor: Colors.transparent,
        foregroundColor: Colors.white,
        actions: [
          IconButton(
            tooltip: 'Delete',
            onPressed: () => Navigator.of(context).pop(true),
            icon: const Icon(Icons.delete_outline),
          ),
        ],
      ),
      body: Center(
        child: InteractiveViewer(
          child: Image.file(File(photo.filePath)),
        ),
      ),
    );
  }
}

