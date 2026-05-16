import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/app_list_card.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

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
          AppListCard(
            icon: Icons.route,
            title: 'GPX',
            subtitle: l10n.gpxDescription,
            trailing: [
              IconButton(
                icon: const Icon(Icons.share),
                tooltip: l10n.share,
                onPressed: () =>
                    _export(context, ExportFormat.gpx, share: true),
              ),
              IconButton(
                icon: const Icon(Icons.save_alt),
                tooltip: l10n.saveToFile,
                onPressed: () =>
                    _export(context, ExportFormat.gpx, share: false),
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
                    _export(context, ExportFormat.geoJson, share: true),
              ),
              IconButton(
                icon: const Icon(Icons.save_alt),
                tooltip: l10n.saveToFile,
                onPressed: () =>
                    _export(context, ExportFormat.geoJson, share: false),
              ),
            ],
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
    final navigator = Navigator.of(context);

    try {
      final service = PathExportService();
      if (share) {
        await service.share(path, format);
      } else {
        await service.saveToFile(path, format);
      }

      if (!context.mounted) return;
      navigator.pop();
      AppSnackbars.success(context, l10n.pathExported);
    } catch (error) {
      if (!context.mounted) return;
      AppSnackbars.error(context, l10n.errorExportingPath(error.toString()));
    }
  }
}

