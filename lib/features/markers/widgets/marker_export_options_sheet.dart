import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/sharing/api.dart';
import 'package:turbo/core/widgets/app_list_card.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

import '../data/marker_export_service.dart';
import '../models/marker.dart';

class MarkerExportOptionsSheet extends ConsumerWidget {
  final Marker marker;

  const MarkerExportOptionsSheet({super.key, required this.marker});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
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
          AppListCard(
            icon: Icons.link,
            title: l10n.shareAsLink,
            subtitle: l10n.linkDescription,
            trailing: [
              IconButton(
                icon: const Icon(Icons.share),
                tooltip: l10n.share,
                onPressed: () => _shareLink(context, ref),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _shareLink(BuildContext context, WidgetRef ref) async {
    final l10n = context.l10n;
    final navigator = Navigator.of(context);
    final webBaseUrl = ref.read(webBaseUrlProvider);

    try {
      await MarkerExportService().shareAsLink(marker, webBaseUrl);
      if (!context.mounted) return;
      navigator.pop();
      AppSnackbars.success(context, l10n.linkCopied);
    } catch (error) {
      if (!context.mounted) return;
      AppSnackbars.error(context, l10n.errorExportingMarker(error.toString()));
    }
  }
}
