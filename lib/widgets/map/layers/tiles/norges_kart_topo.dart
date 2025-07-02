import 'package:dio_cache_interceptor_hive_store/dio_cache_interceptor_hive_store.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_cache/flutter_map_cache.dart';
import 'package:flutter_map_cancellable_tile_provider/flutter_map_cancellable_tile_provider.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/widgets/map/layers/tiles/tile_registry/tile_provider.dart';

class NorgeskartProvider extends TileProviderWrapper {
  final  String? cachePath;

  NorgeskartProvider({required this.cachePath});

  @override String get id => 'topo';
  @override String name(BuildContext context) => context.l10n.layerNameNorgeskart;
  @override String description(BuildContext context) => context.l10n.layerDescriptionNorgeskart;
  @override String get attributions => 'Norgeskart';
  @override TileCategory get category => TileCategory.local;

  @override
  TileLayer createTileLayer() => TileLayer(
    urlTemplate: 'https://cache.atgcp1-prod.kartverket.cloud/v1/service?layer=topo&style=default&tilematrixset=webmercator&Service=WMTS&Request=GetTile&Version=1.0.0&Format=image/png&TileMatrix={z}&TileCol={x}&TileRow={y}',
    panBuffer: 2, // Pre-load tiles when panning to improve perceived performance
    evictErrorTileStrategy: EvictErrorTileStrategy.none,
    tileProvider: cachePath != null
        ? CachedTileProvider(
      maxStale: const Duration(days: 30),
      store: HiveCacheStore(
        cachePath,
        hiveBoxName: 'norgeskart_topo_cache',
      ),
    )
        : CancellableNetworkTileProvider(),
    errorTileCallback: (tile, error, stackTrace) {
      if (kDebugMode) {
        // print('Failed to load tile ${tile.coordinates} from $id: $error');
      }
    },
  );
}