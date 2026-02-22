import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart'
as offline_regions_api;
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/widgets/map/buttons/map_control_button_base.dart';

class MapLayerButton extends ConsumerWidget {
  const MapLayerButton({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Ensure providers are initialized before showing the sheet.
    ref.watch(tileRegistryProvider);

    return MapControlButtonBase(
      child: Icon(
        Icons.layers,
        color: Theme.of(context).colorScheme.primary,
      ),
      onPressed: () => _showBottomSheet(context),
    );
  }

  void _showBottomSheet(BuildContext context) {
    showModalBottomSheet(
      context: context,
      backgroundColor: Colors.transparent,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (BuildContext context) {
        return const LayerSelectionSheet();
      },
    );
  }
}

class LayerSelectionSheet extends ConsumerWidget {
  const LayerSelectionSheet({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final registry = ref.watch(tileRegistryProvider);
    final globalLayers = ref.watch(globalLayersProvider);
    final localLayers = ref.watch(localLayersProvider);
    final overlayLayers = ref.watch(overlayLayersProvider);
    final offlineLayers = ref.watch(offlineLayersProvider);
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    // Determine which layers are currently active
    final activeLayerIds = {
      ...registry.activeGlobalIds,
      ...registry.activeLocalIds,
      ...registry.activeOverlayIds,
      ...registry.activeOfflineIds,
    };

    return SafeArea(
      child: Container(
        decoration: BoxDecoration(
          color: colorScheme.surface,
          borderRadius: const BorderRadius.vertical(top: Radius.circular(28)),
        ),
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const SizedBox(height: 12),
              Center(
                child: Container(
                  width: 32,
                  height: 4,
                  decoration: BoxDecoration(
                    color: colorScheme.outlineVariant,
                    borderRadius: BorderRadius.circular(2),
                  ),
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(24, 24, 24, 16),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text(
                      l10n.mapLayers,
                      style: textTheme.headlineSmall,
                    ),
                    IconButton(
                      icon: const Icon(Icons.close),
                      onPressed: () => Navigator.of(context).pop(),
                      style: IconButton.styleFrom(
                        foregroundColor: colorScheme.onSurfaceVariant,
                        backgroundColor: colorScheme.surfaceContainerHighest,
                      ),
                    ),
                  ],
                ),
              ),
              _buildLayerSection(
                context,
                ref,
                title: l10n.globalMaps,
                layers: globalLayers,
                activeLayerIds: activeLayerIds,
                onToggle: (id) =>
                    ref.read(tileRegistryProvider.notifier).toggleGlobalLayer(id),
              ),
              _buildLayerSection(
                context,
                ref,
                title: l10n.norwegianMaps,
                layers: localLayers,
                activeLayerIds: activeLayerIds,
                onToggle: (id) =>
                    ref.read(tileRegistryProvider.notifier).toggleLocalLayer(id),
              ),
              _buildLayerSection(
                context,
                ref,
                title: l10n.overlays,
                layers: overlayLayers,
                activeLayerIds: activeLayerIds,
                onToggle: (id) =>
                    ref.read(tileRegistryProvider.notifier).toggleOverlay(id),
              ),
              const Divider(height: 24, indent: 24, endIndent: 24),
              _buildLayerSection(
                context,
                ref,
                title: "Offline Maps",
                layers: offlineLayers,
                activeLayerIds: activeLayerIds,
                onToggle: (id) => ref
                    .read(tileRegistryProvider.notifier)
                    .toggleOfflineLayer(id),
                isOffline: true,
              ),
              const SizedBox(height: 24),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildLayerSection(
      BuildContext context,
      WidgetRef ref, {
        required String title,
        required List<TileProviderConfig> layers,
        required Set<String> activeLayerIds,
        required void Function(String) onToggle,
        bool isOffline = false,
      }) {
    if (layers.isEmpty && !isOffline) {
      return const SizedBox.shrink();
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 24),
          child: _buildSectionHeader(context, title),
        ),
        const SizedBox(height: 16),
        if (layers.isNotEmpty)
          SizedBox(
            height: 120,
            child: ListView.builder(
              padding: const EdgeInsets.symmetric(horizontal: 24),
              scrollDirection: Axis.horizontal,
              itemCount: layers.length,
              itemBuilder: (context, index) {
                final layer = layers[index];
                return _buildLayerCard(
                  context: context,
                  label: layer.name(context),
                  value: layer.id,
                  isSelected: activeLayerIds.contains(layer.id),
                  icon: _getLayerIcon(layer),
                  onToggle: () => onToggle(layer.id),
                );
              },
            ),
          )
        else if (isOffline)
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 24, vertical: 24),
            child: Center(child: Text("No offline maps downloaded yet.")),
          ),
        if (isOffline) ...[
          const SizedBox(height: 16),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 24),
            child: Row(
              children: [
                Expanded(
                  child: FilledButton.tonal(
                    onPressed: () {
                      Navigator.of(context).pop(); // Close sheet
                      Navigator.of(context).push(MaterialPageRoute(
                        builder: (context) =>
                        const offline_regions_api.OfflineRegionsPage(),
                      ));
                    },
                    child: const Text("Manage"),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: FilledButton(
                    onPressed: () {
                      final mapState = ref.read(mapViewStateProvider);
                      final activeLayers = ref.read(activeTileLayersProvider);
                      Navigator.of(context).pop(); // Close sheet
                      Navigator.of(context).push(MaterialPageRoute(
                        builder: (_) =>
                            offline_regions_api.RegionCreationPage(
                              initialCenter: mapState.center,
                              initialZoom: mapState.zoom,
                              activeTileLayer:
                              activeLayers.isNotEmpty ? activeLayers.first : null,
                            ),
                      ));
                    },
                    child: const Text("Download"),
                  ),
                ),
              ],
            ),
          ),
        ],
        const SizedBox(height: 24),
      ],
    );
  }

  Widget _buildSectionHeader(BuildContext context, String title) {
    return Text(
      title,
      style: Theme.of(context).textTheme.titleMedium?.copyWith(
        color: Theme.of(context).colorScheme.onSurfaceVariant,
        fontWeight: FontWeight.w600,
      ),
    );
  }

  Widget _buildLayerCard({
    required BuildContext context,
    required String label,
    required String value,
    required bool isSelected,
    required VoidCallback onToggle,
    required IconData icon,
  }) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Padding(
      padding: const EdgeInsets.only(right: 12),
      child: SizedBox(
        width: 100,
        child: InkWell(
          onTap: onToggle,
          borderRadius: BorderRadius.circular(12),
          child: Container(
            padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 8),
            decoration: BoxDecoration(
              color: isSelected
                  ? colorScheme.secondaryContainer
                  : colorScheme.surfaceContainer,
              borderRadius: BorderRadius.circular(12),
              border: Border.all(
                color: isSelected
                    ? colorScheme.secondary
                    : colorScheme.outline.withValues(alpha: 0.2),
                width: isSelected ? 1.5 : 1.0,
              ),
            ),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Icon(
                  icon,
                  size: 28,
                  color: isSelected
                      ? colorScheme.onSecondaryContainer
                      : colorScheme.onSurfaceVariant,
                ),
                const SizedBox(height: 8),
                Text(
                  label,
                  textAlign: TextAlign.center,
                  style: textTheme.bodySmall?.copyWith(
                    fontWeight:
                    isSelected ? FontWeight.bold : FontWeight.normal,
                    color: isSelected
                        ? colorScheme.onSecondaryContainer
                        : colorScheme.onSurfaceVariant,
                  ),
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  IconData _getLayerIcon(TileProviderConfig layer) {
    if (layer.category == TileProviderCategory.offline) {
      return Icons.download_done;
    }
    switch (layer.id) {
      case 'topo':
        return Icons.terrain;
      case 'osm':
        return Icons.map;
      case 'gs':
        return Icons.satellite;
      case 'avalanche_danger':
        return Icons.ac_unit;
      default:
        return Icons.layers;
    }
  }
}