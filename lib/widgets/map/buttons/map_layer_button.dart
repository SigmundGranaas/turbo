import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/widgets/map/layers/tiles/tile_registry/tile_registry.dart';

import '../../../data/state/providers/initialize_tiles_provider.dart';

class MapLayerButton extends ConsumerWidget {
  const MapLayerButton({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Card(
      elevation: 4,
      child: Padding(
        padding: const EdgeInsets.all(8),
        child: IconButton(
          icon: const Icon(Icons.layers),
          onPressed: () => _showBottomSheet(context),
        ),
      ),
    );
  }

  void _showBottomSheet(BuildContext context) {
    showModalBottomSheet(
      context: context,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
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

    return Container(
      padding: const EdgeInsets.all(16),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text(
                  'Velg kartlag',
                  style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)
              ),
              IconButton(
                icon: const Icon(Icons.close),
                onPressed: () => Navigator.of(context).pop(),
              ),
            ],
          ),
          const SizedBox(height: 16),
          const Text(
              'Globalt',
              style: TextStyle(fontWeight: FontWeight.bold)
          ),
          const SizedBox(height: 8),
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: [
                for (final layer in globalLayers)
                  _buildLayerCard(
                    label: layer.name,
                    value: layer.id,
                    isSelected: registry.selectedGlobalId == layer.id,
                    onToggle: () {
                      final notifier = ref.read(tileRegistryProvider.notifier);
                      if (registry.selectedGlobalId == layer.id) {
                        notifier.setGlobalLayer('');  // Deselect
                      } else {
                        notifier.setGlobalLayer(layer.id);
                      }
                    },
                  ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          const Text(
              'Norge',
              style: TextStyle(fontWeight: FontWeight.bold)
          ),
          const SizedBox(height: 8),
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: [
                for (final layer in localLayers)
                  _buildLayerCard(
                    label: layer.name,
                    value: layer.id,
                    isSelected: registry.activeLocalIds.contains(layer.id),
                    onToggle: () {
                      ref.read(tileRegistryProvider.notifier)
                          .toggleLocalLayer(layer.id);
                    },
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildLayerCard({
    required String label,
    required String value,
    required bool isSelected,
    required VoidCallback onToggle,
  }) {
    return Column(
      children: [
        Card(
          elevation: 2,
          color: isSelected ? Colors.blue.shade100 : Colors.white,
          child: Padding(
            padding: const EdgeInsets.all(8.0),
            child: IconButton(
              onPressed: onToggle,
              icon: const Icon(Icons.layers),
            ),
          ),
        ),
        Padding(
          padding: const EdgeInsets.all(8.0),
          child: Text(
            label,
            style: const TextStyle(fontWeight: FontWeight.bold),
          ),
        ),
      ],
    );
  }
}
