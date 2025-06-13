import 'package:turbo/data/layer_preference_service.dart';
import 'package:turbo/data/state/providers/tile_cache_path_provider.dart';
import 'package:turbo/widgets/map/layers/tiles/avalanche_overlay.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../widgets/map/layers/tiles/osm_tiles.dart';
import '../../../widgets/map/layers/tiles/google_sattelite.dart';
import '../../../widgets/map/layers/tiles/norges_kart_topo.dart';
import '../../../widgets/map/layers/tiles/tile_registry/tile_registry.dart';

part 'initialize_tiles_provider.g.dart';

@riverpod
Future<void> initializeTiles(InitializeTilesRef ref) async {
  final registry = ref.read(tileRegistryProvider.notifier);
  final prefsService = ref.read(layerPreferenceServiceProvider);
  final cachePath = await ref.watch(cachePathProvider.future);

  // Register all available providers
  registry
    ..registerProvider(NorgeskartProvider(cachePath: cachePath))
    ..registerProvider(OSMProvider(cachePath: cachePath))
    ..registerProvider(GoogleSatellite(cachePath: cachePath))
    ..registerProvider(AvalancheOverlay(cachePath: cachePath));

  // Load saved layer preferences
  final savedLayers = await prefsService.getSavedLayers();
  final hasSavedLocal = savedLayers['local']?.isNotEmpty ?? false;
  final hasSavedGlobal = savedLayers['global']?.isNotEmpty ?? false;

  // If preferences exist, apply them. Otherwise, set a default.
  if (hasSavedLocal || hasSavedGlobal) {
    registry.initializeWith(
      savedLayers['global']!,
      savedLayers['local']!,
      savedLayers['overlays']!,
    );
  } else {
    // This will be called on first launch, and it will persist the state
    registry.toggleLocalLayer('topo');
  }
}