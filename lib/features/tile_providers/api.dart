library;

/// The public API for the Tile Providers feature.
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/tile_providers/data/tile_registry.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
import 'package:turbo/features/tile_providers/models/tile_registry_state.dart';

// 1. Export the public models.
export 'package:turbo/features/tile_providers/models/tile_provider_config.dart';
export 'package:turbo/features/tile_providers/models/tile_registry_state.dart';
export 'package:turbo/features/tile_providers/data/providers/osm_tiles.dart' show OsmConfig;

// 2. Export the main state notifier provider for the registry.
final tileRegistryProvider =
NotifierProvider<TileRegistry, TileRegistryState>(TileRegistry.new);

// 3. Provider for the active list of TileLayer widgets to be displayed on the map.
final activeTileLayersProvider = Provider<List<TileLayer>>((ref) {
  // Watching the registry ensures this provider rebuilds when active layers change.
  ref.watch(tileRegistryProvider);
  // Reading the notifier to call the method that generates the layers.
  return ref.read(tileRegistryProvider.notifier).getActiveLayers();
});

// 4. Convenience providers for filtering available layers by category for the UI.

/// Provides a list of all available "global" map layers.
final globalLayersProvider = Provider<List<TileProviderConfig>>((ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((p) => p.category == TileProviderCategory.global)
      .toList();
});

/// Provides a list of all available "local" (e.g., country-specific) map layers.
/// This does NOT include offline maps.
final localLayersProvider = Provider<List<TileProviderConfig>>((ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((p) => p.category == TileProviderCategory.local)
      .toList();
});

/// Provides a list of all available overlay layers.
final overlayLayersProvider = Provider<List<TileProviderConfig>>((ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((p) => p.category == TileProviderCategory.overlay)
      .toList();
});

/// Provides a list of all downloaded offline map layers.
final offlineLayersProvider = Provider<List<TileProviderConfig>>((ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((p) => p.category == TileProviderCategory.offline)
      .toList();
});