import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_section_header.dart';
import 'package:turbo/features/map_view/api.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/photo_map/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart'
as offline_regions_api;
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/map/buttons/map_control_button_base.dart';

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
      child: Material(
        color: colorScheme.surface,
        borderRadius:
            const BorderRadius.vertical(top: Radius.circular(AppRadius.xl)),
        clipBehavior: Clip.antiAlias,
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
                    // Drag-handle pill is intentionally tiny; localized here.
                    borderRadius: const BorderRadius.all(Radius.circular(2)),
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
              _buildDataSection(context, ref),
              const Divider(height: 24, indent: 24, endIndent: 24),
              _buildLayerSection(
                context,
                ref,
                title: l10n.offlineMaps,
                layers: offlineLayers,
                activeLayerIds: activeLayerIds,
                onToggle: (id) => ref
                    .read(tileRegistryProvider.notifier)
                    .toggleOfflineLayer(id),
                isOffline: true,
              ),
              const Divider(height: 24, indent: 24, endIndent: 24),
              _buildAddCustomMapTile(context),
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
          Padding(
            padding: const EdgeInsets.symmetric(
                horizontal: AppSpacing.xl, vertical: AppSpacing.xl),
            child: Center(child: Text(context.l10n.noOfflineMapsDownloaded)),
          ),
        if (isOffline) ...[
          const SizedBox(height: AppSpacing.l),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: AppSpacing.xl),
            child: Row(
              children: [
                Expanded(
                  child: AppButton.tonal(
                    text: context.l10n.manage,
                    fullWidth: true,
                    onPressed: () {
                      Navigator.of(context).pop();
                      Navigator.of(context).push(MaterialPageRoute(
                        builder: (context) =>
                            const offline_regions_api.OfflineRegionsPage(),
                      ));
                    },
                  ),
                ),
                const SizedBox(width: AppSpacing.m),
                Expanded(
                  child: AppButton.primary(
                    text: context.l10n.download,
                    fullWidth: true,
                    onPressed: () {
                      final mapState = ref.read(mapViewStateProvider);
                      final activeLayers = ref.read(activeTileLayersProvider);
                      Navigator.of(context).pop();
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

  Widget _buildDataSection(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final markersVisible = ref.watch(markersVisibleProvider);
    final pathsVisible = ref.watch(savedPathsVisibleProvider);
    final photosVisible = ref.watch(photoLayerVisibleProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 24),
          child: _buildSectionHeader(context, l10n.dataLayers),
        ),
        SwitchListTile(
          contentPadding: const EdgeInsets.symmetric(horizontal: 24),
          secondary: const Icon(Icons.location_on_outlined),
          title: Text(l10n.showMarkers),
          value: markersVisible,
          onChanged: (_) => ref.read(markersVisibleProvider.notifier).toggle(),
        ),
        SwitchListTile(
          contentPadding: const EdgeInsets.symmetric(horizontal: 24),
          secondary: const Icon(Icons.route_outlined),
          title: Text(l10n.showPaths),
          value: pathsVisible,
          onChanged: (_) => ref.read(savedPathsVisibleProvider.notifier).toggle(),
        ),
        SwitchListTile(
          contentPadding: const EdgeInsets.symmetric(horizontal: 24),
          secondary: const Icon(Icons.photo_camera_outlined),
          title: Text(l10n.showPhotos),
          subtitle: Text(l10n.showPhotosSubtitle),
          value: photosVisible,
          onChanged: (_) =>
              ref.read(photoLayerVisibleProvider.notifier).toggle(),
        ),
        const SizedBox(height: 8),
      ],
    );
  }

  Widget _buildSectionHeader(BuildContext context, String title) {
    return AppSectionHeader(title);
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
          borderRadius: BorderRadius.circular(AppRadius.m),
          child: Container(
            padding: const EdgeInsets.symmetric(vertical: AppSpacing.s, horizontal: AppSpacing.s),
            decoration: BoxDecoration(
              color: isSelected
                  ? colorScheme.secondaryContainer
                  : colorScheme.surfaceContainer,
              borderRadius: BorderRadius.circular(AppRadius.m),
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

  Widget _buildAddCustomMapTile(BuildContext context) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    return ListTile(
      contentPadding: const EdgeInsets.symmetric(horizontal: 24),
      leading: Icon(Icons.add_link, color: colorScheme.primary),
      title: Text(l10n.addCustomMap),
      subtitle: Text(
        l10n.customMapUrlHelp,
        style: Theme.of(context).textTheme.bodySmall,
      ),
      onTap: () {
        // Close the layer-selection sheet first so the new page replaces
        // it instead of stacking on top of a modal scrim.
        Navigator.of(context).pop();
        pushAddCustomMapPage(context);
      },
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
      case 'ocean_conditions':
        return Icons.waves;
      default:
        return Icons.layers;
    }
  }
}