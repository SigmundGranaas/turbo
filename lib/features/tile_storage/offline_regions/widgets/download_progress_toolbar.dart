import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_pill.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

class DownloadProgressToolbar extends ConsumerWidget {
  final OfflineRegion region;
  final VoidCallback onHide;

  const DownloadProgressToolbar({
    super.key,
    required this.region,
    required this.onHide,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final l10n = context.l10n;
    return AppCardSurface(
      maxWidth: 500,
      padding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.l, vertical: AppSpacing.m),
      child: Row(
        children: [
          SizedBox(
            width: 40,
            height: 40,
            child: Stack(
              fit: StackFit.expand,
              children: [
                CircularProgressIndicator(
                  value: region.progress,
                  strokeWidth: 4,
                  backgroundColor: theme.colorScheme.surfaceContainerHighest,
                ),
                Center(
                  child: Text(
                    "${(region.progress * 100).toInt()}%",
                    style: theme.textTheme.labelSmall
                        ?.copyWith(fontWeight: FontWeight.bold),
                  ),
                )
              ],
            ),
          ),
          const SizedBox(width: AppSpacing.l),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  region.name,
                  style: theme.textTheme.titleMedium,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                const SizedBox(height: AppSpacing.xs),
                Text(
                    l10n.tilesDownloaded(region.downloadedTiles, region.totalTiles),
                    style: theme.textTheme.bodySmall),
              ],
            ),
          ),
          IconButton(
            icon: const Icon(Icons.close),
            tooltip: l10n.hide,
            onPressed: onHide,
          )
        ],
      ),
    );
  }
}