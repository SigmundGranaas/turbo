import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/map_view/data/norway_utm33_crs.dart';
import 'package:turbo/features/tile_providers/data/tile_registry.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

/// The projection the map should render in, derived from the active layers.
///
/// We switch to UTM33 (high-detail Norwegian topo) only when topo is the sole
/// base: any active Web Mercator base (satellite/OSM) or offline region keeps
/// the map in Web Mercator so those raster tiles stay aligned.
final mapProjectionProvider = Provider<MapProjection>((ref) {
  final state = ref.watch(tileRegistryProvider);
  final topoIsBase = state.activeLocalIds.contains('topo') &&
      state.activeGlobalIds.isEmpty &&
      state.activeOfflineIds.isEmpty;
  return topoIsBase ? MapProjection.utm33 : MapProjection.webMercator;
});

/// The active list of [TileLayer] widgets to be displayed on the map.
///
/// Rebuilds whenever the registry's active layer set or the derived
/// projection changes.
final activeTileLayersProvider = Provider<List<TileLayer>>((ref) {
  ref.watch(tileRegistryProvider);
  final projection = ref.watch(mapProjectionProvider);
  return ref
      .read(tileRegistryProvider.notifier)
      .getActiveLayers(projection: projection);
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
