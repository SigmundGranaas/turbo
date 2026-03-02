import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/widgets/action_button.dart';
import 'package:turbo/l10n/app_localizations.dart';
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

          const SizedBox(height: 24),

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

  void _confirmDelete() {
    final l10n = context.l10n;
    showDialog(
      context: context,
      builder: (dialogContext) => AlertDialog.adaptive(
        title: Text(l10n.confirmDeleteTitle),
        content: Text(l10n.confirmDeleteMessage),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: Theme.of(context).colorScheme.error,
              foregroundColor: Theme.of(context).colorScheme.onError,
            ),
            onPressed: () {
              Navigator.of(dialogContext).pop();
              _deleteMarker();
            },
            child: Text(l10n.delete),
          ),
        ],
      ),
    );
  }

  Future<void> _deleteMarker() async {
    final l10n = context.l10n;
    setState(() => _isDeleting = true);
    try {
      await ref
          .read(locationRepositoryProvider.notifier)
          .deleteMarker(_marker.uuid);
      if (mounted) {
        Navigator.of(context).pop(MarkerInfoResult.deleted);
      }
    } catch (error) {
      if (mounted) {
        final colorScheme = Theme.of(context).colorScheme;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(l10n.errorDeletingLocation(error.toString())),
            behavior: SnackBarBehavior.floating,
            backgroundColor: colorScheme.errorContainer,
            shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(12)),
            margin: const EdgeInsets.all(16),
          ),
        );
      }
    } finally {
      if (mounted) {
        setState(() => _isDeleting = false);
      }
    }
  }
}

