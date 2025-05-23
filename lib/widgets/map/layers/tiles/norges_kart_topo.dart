import 'package:dio_cache_interceptor_hive_store/dio_cache_interceptor_hive_store.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_cache/flutter_map_cache.dart';
import 'package:flutter_map_cancellable_tile_provider/flutter_map_cancellable_tile_provider.dart';
import 'package:map_app/widgets/map/layers/tiles/tile_registry/tile_provider.dart';

class NorgeskartProvider extends TileProviderWrapper {
  final  String? cachePath;

  NorgeskartProvider({required this.cachePath});

  @override String get id => 'topo';
  @override String get name => 'Norgeskart';
  @override String get description => 'Norwegian Topographic Map';
  @override String get attributions => 'Norgeskart';
  @override TileCategory get category => TileCategory.local;

  @override
  TileLayer createTileLayer() => TileLayer(
      urlTemplate: 'https://cache.atgcp1-prod.kartverket.cloud/v1/service?layer=topo&style=default&tilematrixset=webmercator&Service=WMTS&Request=GetTile&Version=1.0.0&Format=image/png&TileMatrix={z}&TileCol={x}&TileRow={y}',    tileProvider: cachePath != null
        ? CachedTileProvider(
      maxStale: const Duration(days: 30),
      store: HiveCacheStore(
        cachePath,
        hiveBoxName: 'norgeskart_topo_cache',
      ),
    )
        : CancellableNetworkTileProvider(),
  );
}