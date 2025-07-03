import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';

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
    return Card(
      elevation: 4,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(28)),
      color: theme.colorScheme.surfaceContainer,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 500),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
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
                      backgroundColor:
                      theme.colorScheme.surfaceContainerHighest,
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
              const SizedBox(width: 16),
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
                    const SizedBox(height: 4),
                    Text(
                        "${region.downloadedTiles}/${region.totalTiles} tiles downloaded",
                        style: theme.textTheme.bodySmall),
                  ],
                ),
              ),
              IconButton(
                icon: const Icon(Icons.close),
                tooltip: "Hide",
                onPressed: onHide,
              )
            ],
          ),
        ),
      ),
    );
  }
}