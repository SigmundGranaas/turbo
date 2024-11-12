import 'package:dio_cache_interceptor_hive_store/dio_cache_interceptor_hive_store.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_cache/flutter_map_cache.dart';
import 'package:flutter_map_cancellable_tile_provider/flutter_map_cancellable_tile_provider.dart';

import 'tile_registry/tile_provider.dart';

class GoogleSatellite extends TileProviderWrapper {
  final String? cachePath;

  GoogleSatellite({required this.cachePath});

  @override String get id => 'gs';
  @override String get name => 'Google Satellite';
  @override String get description => 'Satellite imagery from Google';
  @override String get attributions => 'Google';
  @override TileCategory get category => TileCategory.global;

  @override
  TileLayer createTileLayer() => TileLayer(
    urlTemplate: 'https://mt0.google.com/vt/lyrs=s&hl=en&x={x}&y={y}&z={z}',
    tileProvider: cachePath != null
        ? CachedTileProvider(
      maxStale: const Duration(days: 30),
      store: HiveCacheStore(
        cachePath,
        hiveBoxName: 'gs_cache',
      ),
    )
        : CancellableNetworkTileProvider(),
  );
}