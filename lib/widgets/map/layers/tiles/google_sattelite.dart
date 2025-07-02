import 'package:dio_cache_interceptor_hive_store/dio_cache_interceptor_hive_store.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_cache/flutter_map_cache.dart';
import 'package:flutter_map_cancellable_tile_provider/flutter_map_cancellable_tile_provider.dart';
import 'package:turbo/l10n/app_localizations.dart';

import 'tile_registry/tile_provider.dart';

class GoogleSatellite extends TileProviderWrapper {
  final String? cachePath;

  GoogleSatellite({required this.cachePath});

  @override String get id => 'gs';
  @override String name(BuildContext context) => context.l10n.layerNameGoogleSatellite;
  @override String description(BuildContext context) => context.l10n.layerDescriptionGoogleSatellite;
  @override String get attributions => 'Google';
  @override TileCategory get category => TileCategory.global;

  @override
  TileLayer createTileLayer() => TileLayer(
    urlTemplate: 'https://mt0.google.com/vt/lyrs=s&hl=en&x={x}&y={y}&z={z}',
    panBuffer: 2,
    evictErrorTileStrategy: EvictErrorTileStrategy.none,
    tileProvider: cachePath != null
        ? CachedTileProvider(
      maxStale: const Duration(days: 30),
      store: HiveCacheStore(
        cachePath,
        hiveBoxName: 'gs_cache',
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