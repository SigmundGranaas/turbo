import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/tile_providers/data/tile_registry.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

/// The active list of [TileLayer] widgets to be displayed on the map.
///
/// Rebuilds whenever the registry's active layer set changes.
final activeTileLayersProvider = Provider<List<TileLayer>>((ref) {
  ref.watch(tileRegistryProvider);
  return ref.read(tileRegistryProvider.notifier).getActiveLayers();
});

/// All available "global" map layers (worldwide basemaps).
final globalLayersProvider = Provider<List<TileProviderConfig>>((ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((p) => p.category == TileProviderCategory.global)
      .toList();
});

/// All available "local" (e.g., country-specific) map layers.
/// This does NOT include offline maps.
final localLayersProvider = Provider<List<TileProviderConfig>>((ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((p) => p.category == TileProviderCategory.local)
      .toList();
});

/// All available overlay layers.
final overlayLayersProvider = Provider<List<TileProviderConfig>>((ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((p) => p.category == TileProviderCategory.overlay)
      .toList();
});

/// All downloaded offline map layers.
final offlineLayersProvider = Provider<List<TileProviderConfig>>((ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((p) => p.category == TileProviderCategory.offline)
      .toList();
});
