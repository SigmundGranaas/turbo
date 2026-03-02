import 'package:flutter/material.dart';
import 'package:turbo/l10n/app_localizations.dart';

import '../data/path_export_service.dart';
import '../models/saved_path.dart';

class ExportOptionsSheet extends StatelessWidget {
  final SavedPath path;

  const ExportOptionsSheet({super.key, required this.path});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final mediaQuery = MediaQuery.of(context);
    final bottomPadding = mediaQuery.viewInsets.bottom > 0
        ? mediaQuery.viewInsets.bottom
        : mediaQuery.padding.bottom;

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomPadding),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(l10n.exportPath, style: textTheme.titleLarge),
              IconButton(
                onPressed: () => Navigator.pop(context),
                icon: const Icon(Icons.close),
              ),
            ],
          ),
          const SizedBox(height: 16),
          _FormatCard(
            icon: Icons.route,
            title: 'GPX',
            description: l10n.gpxDescription,
            shareLabel: l10n.share,
            saveLabel: l10n.saveToFile,
            onShare: () => _export(context, ExportFormat.gpx, share: true),
            onSave: () => _export(context, ExportFormat.gpx, share: false),
          ),
          const SizedBox(height: 12),
          _FormatCard(
            icon: Icons.data_object,
            title: 'GeoJSON',
            description: l10n.geoJsonDescription,
            shareLabel: l10n.share,
            saveLabel: l10n.saveToFile,
            onShare: () => _export(context, ExportFormat.geoJson, share: true),
            onSave: () => _export(context, ExportFormat.geoJson, share: false),
          ),
        ],
      ),
    );
  }

  Future<void> _export(
    BuildContext context,
    ExportFormat format, {
    required bool share,
  }) async {
    final l10n = context.l10n;
    final messenger = ScaffoldMessenger.of(context);
    final navigator = Navigator.of(context);
    final errorColor = Theme.of(context).colorScheme.errorContainer;

    try {
      final service = PathExportService();
      if (share) {
        await service.share(path, format);
      } else {
        await service.saveToFile(path, format);
      }

      navigator.pop();
      messenger.showSnackBar(
        SnackBar(
          content: Text(l10n.pathExported),
          behavior: SnackBarBehavior.floating,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
          margin: const EdgeInsets.all(16),
        ),
      );
    } catch (error) {
      messenger.showSnackBar(
        SnackBar(
          content: Text(l10n.errorExportingPath(error.toString())),
          behavior: SnackBarBehavior.floating,
          backgroundColor: errorColor,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
          margin: const EdgeInsets.all(16),
        ),
      );
    }
  }
}

class _FormatCard extends StatelessWidget {
  final IconData icon;
  final String title;
  final String description;
  final String shareLabel;
  final String saveLabel;
  final VoidCallback onShare;
  final VoidCallback onSave;

  const _FormatCard({
    required this.icon,
    required this.title,
    required this.description,
    required this.shareLabel,
    required this.saveLabel,
    required this.onShare,
    required this.onSave,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Card(
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: BorderSide(color: colorScheme.outlineVariant),
      ),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Row(
          children: [
            Icon(icon, size: 28, color: colorScheme.primary),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title, style: textTheme.titleSmall),
                  const SizedBox(height: 2),
                  Text(
                    description,
                    style: textTheme.bodySmall?.copyWith(
                      color: colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            ),
            IconButton(
              onPressed: onShare,
              icon: const Icon(Icons.share),
              tooltip: shareLabel,
            ),
            IconButton(
              onPressed: onSave,
              icon: const Icon(Icons.save_alt),
              tooltip: saveLabel,
            ),
          ],
        ),
      ),
    );
  }
}
