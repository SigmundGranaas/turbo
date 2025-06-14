import 'package:dio_cache_interceptor_hive_store/dio_cache_interceptor_hive_store.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_cache/flutter_map_cache.dart';
import 'package:flutter_map_cancellable_tile_provider/flutter_map_cancellable_tile_provider.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/widgets/map/layers/tiles/tile_registry/tile_provider.dart';

class AvalancheOverlay extends TileProviderWrapper {
  final  String? cachePath;

  AvalancheOverlay({required this.cachePath});

  @override String get id => 'avalanche_danger';
  @override String name(BuildContext context) => context.l10n.layerNameAvalanche;
  @override String description(BuildContext context) => context.l10n.layerDescriptionAvalanche;
  @override String get attributions => 'NVE';
  @override TileCategory get category => TileCategory.overlay;

  @override
  TileLayer createTileLayer() => TileLayer(
    urlTemplate: 'https://gis3.nve.no/arcgis/rest/services/wmts/Bratthet_med_utlop_2024/MapServer/tile/{z}/{y}/{x}',
    tileDisplay: const TileDisplay.instantaneous(opacity: 0.7),
    tileProvider: cachePath != null
        ? CachedTileProvider(
      maxStale: const Duration(days: 30),
      store: HiveCacheStore(
        cachePath,
        hiveBoxName: 'avalance_danger_cache',
      ),
    )
        : CancellableNetworkTileProvider(),
    errorTileCallback: (tile, error, stackTrace) {
      if (kDebugMode) {
        print('Failed to load tile ${tile.coordinates} from $id: $error');
      }
    },
  );
}