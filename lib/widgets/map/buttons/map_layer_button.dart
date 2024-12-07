import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/widgets/map/buttons/map_control_button_base.dart';
import 'package:map_app/widgets/map/layers/tiles/tile_registry/tile_registry.dart';

class MapLayerButton extends ConsumerWidget {
  const MapLayerButton({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
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
    final registry = ref.watch(tileRegistryProvider);
    final globalLayers = ref.watch(globalLayersProvider);
    final localLayers = ref.watch(localLayersProvider);
    final overlayLayers = ref.watch(overlayLayersProvider);
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Container(
      decoration: BoxDecoration(
        color: colorScheme.surface,
        borderRadius: const BorderRadius.vertical(top: Radius.circular(28)),
      ),
      child: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const SizedBox(height: 12),
            // Drag Handle
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
            // Header Section
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 24, 24, 16),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(
                    'Map Layers',
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

            // Global Maps Section
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 24),
              child: _buildSectionHeader(context, 'Global Maps'),
            ),
            const SizedBox(height: 16),

            SizedBox(
              height: 160,
              child: ListView.builder(
                padding: const EdgeInsets.symmetric(horizontal: 24),
                scrollDirection: Axis.horizontal,
                itemCount: globalLayers.length,
                itemBuilder: (context, index) {
                  final layer = globalLayers[index];
                  final isSelected = registry.activeGlobalIds.contains(layer.id);
                  return _buildLayerCard(
                    context: context,
                    label: layer.name,
                    value: layer.id,
                    isSelected: isSelected,
                    icon: _getLayerIcon(layer.id),
                    onToggle: () {
                      ref.read(tileRegistryProvider.notifier)
                          .toggleGlobalLayer(layer.id);
                    },
                  );
                },
              ),
            ),

            const SizedBox(height: 24),

            // Norwegian Maps Section
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 24),
              child: _buildSectionHeader(context, 'Norwegian Maps'),
            ),
            const SizedBox(height: 16),

            SizedBox(
              height: 160,
              child: ListView.builder(
                padding: const EdgeInsets.symmetric(horizontal: 24),
                scrollDirection: Axis.horizontal,
                itemCount: localLayers.length,
                itemBuilder: (context, index) {
                  final layer = localLayers[index];
                  final isSelected = registry.activeLocalIds.contains(layer.id);
                  return _buildLayerCard(
                    context: context,
                    label: layer.name,
                    value: layer.id,
                    isSelected: isSelected,
                    icon: _getLayerIcon(layer.id),
                    onToggle: () {
                      ref.read(tileRegistryProvider.notifier)
                          .toggleLocalLayer(layer.id);
                    },
                  );
                },
              ),
            ),
            const SizedBox(height: 24),
            _buildSectionHeader(context, 'Overlays'),
            const SizedBox(height: 12),
            SizedBox(
              height: 140,
              child: ListView.builder(
                scrollDirection: Axis.horizontal,
                itemCount: overlayLayers.length,
                itemBuilder: (context, index) {
                  final layer = overlayLayers[index];
                  return _buildLayerCard(
                    context: context,
                    label: layer.name,
                    value: layer.id,
                    isSelected: registry.activeOverlayIds.contains(layer.id),
                    icon: _getLayerIcon(layer.id),
                    onToggle: () {
                      ref.read(tileRegistryProvider.notifier)
                          .toggleOverlay(layer.id);
                    },
                  );
                },
              ),
            ),
            const SizedBox(height: 16),
          ],
        ),
      ),
    );
  }

  Widget _buildSectionHeader(BuildContext context, String title) {
    return Align(
      alignment: Alignment.centerLeft,
      child: Text(
        title,
        style: Theme.of(context).textTheme.titleMedium?.copyWith(
          color: Theme.of(context).colorScheme.onSurfaceVariant,
        ),
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
      padding: const EdgeInsets.only(right: 16),
      child: Card(
        elevation: 0,
        color: isSelected ? colorScheme.secondaryContainer : colorScheme.surfaceContainerLow,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(16),
          side: BorderSide(
            color: isSelected ? colorScheme.outline : Colors.transparent,
            width: isSelected ? 1 : 0,
          ),
        ),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: onToggle,
          splashColor: colorScheme.secondaryContainer.withValues(alpha: 0.3),
          child: Container(
            width: 120,
            padding: const EdgeInsets.all(16),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                const Spacer(),
                Icon(
                  icon,
                  size: 36,
                  color: isSelected
                      ? colorScheme.onSecondaryContainer
                      : colorScheme.onSurfaceVariant,
                ),
                const SizedBox(height: 16),
                Text(
                  label,
                  textAlign: TextAlign.center,
                  style: textTheme.labelLarge?.copyWith(
                    fontWeight: isSelected ? FontWeight.w500 : FontWeight.normal,
                    color: isSelected
                        ? colorScheme.onSecondaryContainer
                        : colorScheme.onSurfaceVariant,
                  ),
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                ),
                const Spacer(),
              ],
            ),
          ),
        ),
      ),
    );
  }

  IconData _getLayerIcon(String layerId) {
    switch (layerId) {
      case 'topo':
        return Icons.terrain;
      case 'osm':
        return Icons.map;
      case 'gs':
        return Icons.satellite;
      default:
        return Icons.layers;
    }
  }
}