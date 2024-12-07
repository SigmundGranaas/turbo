import 'package:map_app/data/state/providers/tile_cache_path_provider.dart';
import 'package:map_app/widgets/map/layers/tiles/avalanche_overlay.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../widgets/map/layers/tiles/osm_tiles.dart';
import '../../../widgets/map/layers/tiles/google_sattelite.dart';
import '../../../widgets/map/layers/tiles/norges_kart_topo.dart';
import '../../../widgets/map/layers/tiles/tile_registry/tile_registry.dart';

part 'initialize_tiles_provider.g.dart';

@riverpod
Future<void> initializeTiles(InitializeTilesRef ref) async {
  final registry = ref.read(tileRegistryProvider.notifier);
  final cachePath = await ref.watch(cachePathProvider.future);

  registry
    ..registerProvider(NorgeskartProvider(cachePath: cachePath))
    ..registerProvider(OSMProvider(cachePath: cachePath))
    ..registerProvider(GoogleSatellite(cachePath: cachePath))
    ..registerProvider(AvalancheOverlay(cachePath: cachePath));

  registry.toggleLocalLayer('topo');
}
