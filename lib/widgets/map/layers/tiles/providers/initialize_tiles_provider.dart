import 'package:map_app/widgets/map/layers/tiles/google_sattelite.dart';
import 'package:map_app/widgets/map/layers/tiles/providers/tile_cache_path_provider.dart';
import 'package:map_app/widgets/map/layers/tiles/registry/tile_registry.dart';
import 'package:map_app/widgets/map/layers/tiles/providers/tile_provider.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../OSM_tiles.dart';
import '../norges_kart_topo.dart';

part 'initialize_tiles_provider.g.dart';

@riverpod
Future<void> initializeTiles(InitializeTilesRef ref) async {
  final registry = ref.read(tileRegistryProvider.notifier);
  final cachePath = await ref.watch(cachePathProvider.future);

  registry
    ..registerProvider(NorgeskartProvider(cachePath: cachePath))
    ..registerProvider(OSMProvider(cachePath: cachePath))
    ..registerProvider(GoogleSatellite(cachePath: cachePath));
}

@riverpod
List<TileProviderWrapper> globalLayers(GlobalLayersRef ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((provider) => provider.category == TileCategory.global)
      .toList();
}

@riverpod
List<TileProviderWrapper> localLayers(LocalLayersRef ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((provider) => provider.category == TileCategory.local)
      .toList();
}

@riverpod
List<TileProviderWrapper> overlayLayers(OverlayLayersRef ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((provider) => provider.category == TileCategory.overlay)
      .toList();
}

