import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/widgets/action_button.dart';
import 'package:turbo/features/markers/data/icon_service.dart';
import 'package:turbo/l10n/app_localizations.dart';
import '../data/saved_path_repository.dart';
import '../models/path_style.dart';
import '../models/saved_path.dart';
import 'export_options_sheet.dart';
import 'path_detail_sheet.dart';

class PathInfoSheet extends ConsumerStatefulWidget {
  final SavedPath path;

  const PathInfoSheet({super.key, required this.path});

  @override
  ConsumerState<PathInfoSheet> createState() => _PathInfoSheetState();
}

class _PathInfoSheetState extends ConsumerState<PathInfoSheet> {
  late SavedPath _path;
  bool _isDeleting = false;

  @override
  void initState() {
    super.initState();
    _path = widget.path;
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

    final pathColor = hexToColor(_path.colorHex) ?? colorScheme.onSurfaceVariant;
    final hasIcon = _path.iconKey != null;
    final namedIcon =
        hasIcon ? IconService().getIcon(context, _path.iconKey) : null;

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
                  color: pathColor.withAlpha(30),
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  hasIcon ? namedIcon!.icon : Icons.route,
                  color: pathColor,
                  size: 24,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  _path.title,
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
              Icon(Icons.straighten,
                  size: 18, color: colorScheme.onSurfaceVariant),
              const SizedBox(width: 8),
              Text(
                '${l10n.totalDistance}: ${(_path.distance / 1000).toStringAsFixed(2)} km',
                style: textTheme.bodyMedium?.copyWith(
                  color: colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
          const SizedBox(height: 4),
          Row(
            children: [
              Icon(Icons.calendar_today_outlined,
                  size: 18, color: colorScheme.onSurfaceVariant),
              const SizedBox(width: 8),
              Text(
                '${l10n.createdDate}: '
                '${_path.createdAt.day.toString().padLeft(2, '0')}.'
                '${_path.createdAt.month.toString().padLeft(2, '0')}.'
                '${_path.createdAt.year}',
                style: textTheme.bodyMedium?.copyWith(
                  color: colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),

          if (_path.description != null &&
              _path.description!.isNotEmpty) ...[
            const SizedBox(height: 8),
            Text(
              _path.description!,
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
    final result = await showModalBottomSheet<PathDetailResult>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (_) => PathDetailSheet(path: _path),
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
      builder: (_) => ExportOptionsSheet(path: _path),
    );
  }

  void _confirmDelete() {
    final l10n = context.l10n;
    showDialog(
      context: context,
      builder: (dialogContext) => AlertDialog.adaptive(
        title: Text(l10n.confirmDeletePathTitle),
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
              _deletePath();
            },
            child: Text(l10n.delete),
          ),
        ],
      ),
    );
  }

  Future<void> _deletePath() async {
    final l10n = context.l10n;
    setState(() => _isDeleting = true);
    try {
      await ref
          .read(savedPathRepositoryProvider.notifier)
          .deletePath(_path.uuid);
      if (mounted) {
        Navigator.of(context).pop(PathDetailResult.deleted);
      }
    } catch (error) {
      if (mounted) {
        final colorScheme = Theme.of(context).colorScheme;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(l10n.errorDeletingPath(error.toString())),
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
