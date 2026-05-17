import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/app_list_card.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

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
                tooltip: l10n.close,
              ),
            ],
          ),
          const SizedBox(height: 16),
          AppListCard(
            icon: Icons.text_fields,
            title: l10n.shareAsText,
            subtitle: l10n.textDescription,
            trailing: [
              IconButton(
                icon: const Icon(Icons.share),
                tooltip: l10n.share,
                onPressed: () => _export(context, _MarkerExportAction.shareText),
              ),
            ],
          ),
          const SizedBox(height: 12),
          AppListCard(
            icon: Icons.data_object,
            title: 'GeoJSON',
            subtitle: l10n.geoJsonDescription,
            trailing: [
              IconButton(
                icon: const Icon(Icons.share),
                tooltip: l10n.share,
                onPressed: () =>
                    _export(context, _MarkerExportAction.shareGeoJson),
              ),
              IconButton(
                icon: const Icon(Icons.save_alt),
                tooltip: l10n.saveToFile,
                onPressed: () =>
                    _export(context, _MarkerExportAction.saveGeoJson),
              ),
            ],
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
    final navigator = Navigator.of(context);

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

      if (!context.mounted) return;
      navigator.pop();
      AppSnackbars.success(context, l10n.markerExported);
    } catch (error) {
      if (!context.mounted) return;
      AppSnackbars.error(context, l10n.errorExportingMarker(error.toString()));
    }
  }
}

enum _MarkerExportAction { shareText, shareGeoJson, saveGeoJson }
