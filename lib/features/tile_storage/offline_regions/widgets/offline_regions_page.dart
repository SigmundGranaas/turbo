import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart';

class OfflineRegionsPage extends ConsumerWidget {
  const OfflineRegionsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final offlineRegionsAsync = ref.watch(offlineRegionsProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text("Offline Maps"),
      ),
      body: offlineRegionsAsync.when(
        data: (regions) {
          if (regions.isEmpty) {
            return const Center(
              child: Padding(
                padding: EdgeInsets.all(16.0),
                child: Text(
                  "No maps downloaded yet.\nTap the button below to download an area.",
                  textAlign: TextAlign.center,
                  style: TextStyle(height: 1.5),
                ),
              ),
            );
          }
          return ListView.builder(
            padding: const EdgeInsets.symmetric(vertical: 8),
            itemCount: regions.length,
            itemBuilder: (context, index) {
              final region = regions[index];
              return _RegionListTile(region: region);
            },
          );
        },
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, st) => Center(child: Text("Error: $e")),
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
        label: const Text("Add offline region"),
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
      margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
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
                style: TextStyle(
                    color: colorScheme.error,
                    fontSize: Theme.of(context).textTheme.bodySmall?.fontSize),
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

  void _confirmDelete(
      BuildContext context,
      OfflineRegionsNotifier notifier,
      OfflineRegion region,
      ) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text("Delete ${region.name}?"),
        content:
        const Text("This will remove the offline map data from your device."),
        actions: [
          TextButton(
            child: const Text("Cancel"),
            onPressed: () => Navigator.of(ctx).pop(),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
                backgroundColor: Theme.of(context).colorScheme.error),
            child: const Text("Delete"),
            onPressed: () {
              notifier.deleteRegion(region.id);
              Navigator.of(ctx).pop();
            },
          ),
        ],
      ),
    );
  }

  Color _statusColor(BuildContext context, DownloadStatus status) {
    final colors = Theme.of(context).colorScheme;
    switch (status) {
      case DownloadStatus.downloading:
        return colors.primaryContainer;
      case DownloadStatus.completed:
        return Colors.green.shade100;
      case DownloadStatus.paused:
        return Colors.grey.shade300;
      case DownloadStatus.failed:
        return colors.errorContainer;
      case DownloadStatus.enqueued:
        return colors.secondaryContainer;
    }
  }

  Widget _statusIcon(BuildContext context, OfflineRegion region) {
    final colors = Theme.of(context).colorScheme;
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
              style: TextStyle(
                fontSize: 10,
                fontWeight: FontWeight.bold,
                color: colors.onPrimaryContainer,
              ),
            ),
          ],
        );
      case DownloadStatus.paused:
        return Icon(Icons.pause, color: colors.onSecondaryContainer);
      case DownloadStatus.completed:
        final hasErrors = region.totalTiles > region.downloadedTiles;
        return hasErrors
            ? Icon(Icons.warning_amber, color: colors.error)
            : Icon(Icons.check, color: Colors.green.shade800);
      case DownloadStatus.failed:
        return Icon(Icons.error_outline, color: colors.onErrorContainer);
      case DownloadStatus.enqueued:
        return Icon(Icons.queue, color: colors.onSecondaryContainer);
    }
  }
}