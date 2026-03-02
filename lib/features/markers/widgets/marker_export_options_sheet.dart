import 'package:flutter/material.dart';
import 'package:turbo/l10n/app_localizations.dart';

import '../data/marker_export_service.dart';
import '../models/marker.dart';

class MarkerExportOptionsSheet extends StatelessWidget {
  final Marker marker;

  const MarkerExportOptionsSheet({super.key, required this.marker});

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
              Text(l10n.exportMarker, style: textTheme.titleLarge),
              IconButton(
                onPressed: () => Navigator.pop(context),
                icon: const Icon(Icons.close),
              ),
            ],
          ),
          const SizedBox(height: 16),
          _FormatCard(
            icon: Icons.text_fields,
            title: l10n.shareAsText,
            description: l10n.textDescription,
            shareLabel: l10n.share,
            onShare: () => _export(context, _MarkerExportAction.shareText),
          ),
          const SizedBox(height: 12),
          _FormatCard(
            icon: Icons.data_object,
            title: 'GeoJSON',
            description: l10n.geoJsonDescription,
            shareLabel: l10n.share,
            saveLabel: l10n.saveToFile,
            onShare: () => _export(context, _MarkerExportAction.shareGeoJson),
            onSave: () => _export(context, _MarkerExportAction.saveGeoJson),
          ),
        ],
      ),
    );
  }

  Future<void> _export(
    BuildContext context,
    _MarkerExportAction action,
  ) async {
    final l10n = context.l10n;
    final messenger = ScaffoldMessenger.of(context);
    final navigator = Navigator.of(context);
    final colorScheme = Theme.of(context).colorScheme;

    try {
      final service = MarkerExportService();
      switch (action) {
        case _MarkerExportAction.shareText:
          await service.shareAsText(marker);
        case _MarkerExportAction.shareGeoJson:
          await service.shareAsGeoJson(marker);
        case _MarkerExportAction.saveGeoJson:
          final result = await service.saveToFile(marker);
          if (result == null) return;
      }

      navigator.pop();
      messenger.showSnackBar(
        SnackBar(
          content: Row(
            children: [
              Icon(Icons.check_circle,
                  color: colorScheme.onPrimaryContainer, size: 20),
              const SizedBox(width: 8),
              Text(l10n.markerExported,
                  style: TextStyle(color: colorScheme.onPrimaryContainer)),
            ],
          ),
          backgroundColor: colorScheme.primaryContainer,
          behavior: SnackBarBehavior.floating,
          shape: const StadiumBorder(),
          margin: const EdgeInsets.all(16),
        ),
      );
    } catch (error) {
      messenger.showSnackBar(
        SnackBar(
          content: Text(l10n.errorExportingMarker(error.toString())),
          behavior: SnackBarBehavior.floating,
          backgroundColor: colorScheme.errorContainer,
          shape:
              RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
          margin: const EdgeInsets.all(16),
        ),
      );
    }
  }
}

enum _MarkerExportAction { shareText, shareGeoJson, saveGeoJson }

class _FormatCard extends StatelessWidget {
  final IconData icon;
  final String title;
  final String description;
  final String shareLabel;
  final String? saveLabel;
  final VoidCallback onShare;
  final VoidCallback? onSave;

  const _FormatCard({
    required this.icon,
    required this.title,
    required this.description,
    required this.shareLabel,
    this.saveLabel,
    required this.onShare,
    this.onSave,
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
            if (onSave != null)
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
