import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_dialog.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

class OfflineRegionsPage extends ConsumerWidget {
  const OfflineRegionsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final offlineRegionsAsync = ref.watch(offlineRegionsProvider);

    if (kIsWeb) {
      final colorScheme = Theme.of(context).colorScheme;
      final textTheme = Theme.of(context).textTheme;
      return Scaffold(
        appBar: AppBar(title: Text(l10n.offlineMaps)),
        body: Center(
          child: Padding(
            padding: const EdgeInsets.all(AppSpacing.xl),
            child: Column(
              key: const Key('offline_regions_web_unavailable'),
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(
                  Icons.cloud_off_outlined,
                  size: 56,
                  color: colorScheme.onSurfaceVariant,
                ),
                const SizedBox(height: AppSpacing.l),
                Text(
                  l10n.offlineMapsNotAvailableOnWebTitle,
                  textAlign: TextAlign.center,
                  style: textTheme.titleMedium?.copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: AppSpacing.s),
                Text(
                  l10n.offlineMapsNotAvailableOnWeb,
                  textAlign: TextAlign.center,
                  style: textTheme.bodyMedium?.copyWith(
                    height: 1.45,
                    color: colorScheme.onSurfaceVariant,
                  ),
                ),
              ],
            ),
          ),
        ),
      );
    }

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.offlineMaps),
        actions: [
          PopupMenuButton<int>(
            tooltip: 'Cleanup',
            icon: const Icon(Icons.cleaning_services_outlined),
            onSelected: (days) => _confirmCleanup(context, ref, days),
            itemBuilder: (_) => const [
              PopupMenuItem(value: 30, child: Text('Delete older than 30 days')),
              PopupMenuItem(value: 90, child: Text('Delete older than 90 days')),
              PopupMenuItem(value: 180, child: Text('Delete older than 180 days')),
            ],
          ),
        ],
      ),
      body: offlineRegionsAsync.when(
        data: (regions) {
          if (regions.isEmpty) {
            return Center(
              child: Padding(
                padding: const EdgeInsets.all(AppSpacing.l),
                child: Text(
                  l10n.noOfflineMapsDownloaded,
                  textAlign: TextAlign.center,
                  style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                        height: 1.5,
                        color: Theme.of(context).colorScheme.onSurfaceVariant,
                      ),
                ),
              ),
            );
          }
          return ListView.builder(
            padding: const EdgeInsets.symmetric(vertical: AppSpacing.s),
            itemCount: regions.length,
            itemBuilder: (context, index) {
              final region = regions[index];
              return _RegionListTile(region: region);
            },
          );
        },
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, st) => Center(
          child: Padding(
            padding: const EdgeInsets.all(AppSpacing.l),
            child: Text(
              l10n.errorLoadingOfflineRegions,
              textAlign: TextAlign.center,
            ),
          ),
        ),
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () {
          final mapState = ref.read(mapViewStateProvider);
          final activeLayers = ref.read(activeTileLayersProvider);

          Navigator.of(context).push(MaterialPageRoute(
            builder: (_) => RegionCreationPage(
              initialCenter: mapState.center,
              initialZoom: mapState.zoom,
              activeTileLayer:
              activeLayers.isNotEmpty ? activeLayers.first : null,
            ),
          ));
        },
        label: Text(l10n.addOfflineRegion),
        icon: const Icon(Icons.download_outlined),
      ),
    );
  }
}

class _RegionListTile extends ConsumerWidget {
  final OfflineRegion region;

  const _RegionListTile({required this.region});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final colorScheme = Theme.of(context).colorScheme;
    final notifier = ref.read(offlineRegionsProvider.notifier);

    final failedTiles = (region.status == DownloadStatus.completed)
        ? region.totalTiles - region.downloadedTiles
        : 0;
    final hasErrors = failedTiles > 0;

    return Card(
      margin: const EdgeInsets.symmetric(
          horizontal: AppSpacing.m, vertical: AppSpacing.xs + 2),
      child: ListTile(
        leading: CircleAvatar(
          backgroundColor: _statusColor(context, region.status),
          child: _statusIcon(context, region),
        ),
        title: Text(region.name),
        subtitle: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
                "${region.tileProviderName} (Zoom ${region.minZoom}-${region.maxZoom})"),
            if (region.status == DownloadStatus.downloading)
              Padding(
                padding: const EdgeInsets.only(top: 4),
                child: Text(
                    "Downloading: ${region.downloadedTiles} / ${region.totalTiles}"),
              )
            else
              Text("Completed on ${DateFormat.yMd().format(region.createdAt)}"),
            if (hasErrors)
              Text(
                "Warning: $failedTiles of ${region.totalTiles} tiles could not be downloaded.",
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: colorScheme.error,
                    ),
              ),
            if (region.status == DownloadStatus.downloading)
              Padding(
                padding: const EdgeInsets.only(top: 4.0),
                child: LinearProgressIndicator(
                  value: region.progress,
                ),
              )
          ],
        ),
        trailing: IconButton(
          icon: Icon(Icons.delete_outline, color: colorScheme.error),
          onPressed: () => _confirmDelete(context, notifier, region),
        ),
      ),
    );
  }

  Future<void> _confirmDelete(
    BuildContext context,
    OfflineRegionsNotifier notifier,
    OfflineRegion region,
  ) async {
    final l10n = context.l10n;
    final confirmed = await AppDialog.destructive(
      context,
      title: l10n.deleteRegionTitle(region.name),
      content: l10n.deleteRegionContent,
      destructiveLabel: l10n.delete,
    );
    if (confirmed) {
      notifier.deleteRegion(region.id);
    }
  }

  Color _statusColor(BuildContext context, DownloadStatus status) {
    final colors = Theme.of(context).colorScheme;
    switch (status) {
      case DownloadStatus.downloading:
        return colors.primaryContainer;
      case DownloadStatus.completed:
        return colors.tertiaryContainer;
      case DownloadStatus.paused:
        return colors.surfaceContainerHigh;
      case DownloadStatus.failed:
        return colors.errorContainer;
      case DownloadStatus.enqueued:
        return colors.secondaryContainer;
    }
  }

  Widget _statusIcon(BuildContext context, OfflineRegion region) {
    final colors = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    switch (region.status) {
      case DownloadStatus.downloading:
        return Stack(
          alignment: Alignment.center,
          children: [
            CircularProgressIndicator(
              strokeWidth: 2,
              value: region.progress,
              color: colors.onPrimaryContainer,
            ),
            Text(
              "${(region.progress * 100).toInt()}%",
              style: textTheme.labelSmall?.copyWith(
                fontWeight: FontWeight.bold,
                color: colors.onPrimaryContainer,
              ),
            ),
          ],
        );
      case DownloadStatus.paused:
        return Icon(Icons.pause, color: colors.onSurfaceVariant);
      case DownloadStatus.completed:
        final hasErrors = region.totalTiles > region.downloadedTiles;
        return hasErrors
            ? Icon(Icons.warning_amber, color: colors.error)
            : Icon(Icons.check, color: colors.onTertiaryContainer);
      case DownloadStatus.failed:
        return Icon(Icons.error_outline, color: colors.onErrorContainer);
      case DownloadStatus.enqueued:
        return Icon(Icons.queue, color: colors.onSecondaryContainer);
    }
  }
}

Future<void> _confirmCleanup(
    BuildContext context, WidgetRef ref, int days) async {
  final messenger = ScaffoldMessenger.maybeOf(context);
  final confirmed = await AppDialog.confirm(
    context,
    title: 'Delete old regions?',
    content:
        'This will remove every downloaded region created more than $days days ago.',
    confirmLabel: 'Delete',
  );
  if (!confirmed) return;
  final cutoff = DateTime.now().subtract(Duration(days: days));
  final removed = await ref
      .read(offlineRegionsProvider.notifier)
      .deleteOlderThan(cutoff);
  messenger?.showSnackBar(SnackBar(
    content: Text(removed == 0
        ? 'No regions matched.'
        : 'Deleted $removed region${removed == 1 ? '' : 's'}.'),
  ));
}