import 'package:dio_cache_interceptor_hive_store/dio_cache_interceptor_hive_store.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_cache/flutter_map_cache.dart';
import 'package:flutter_map_cancellable_tile_provider/flutter_map_cancellable_tile_provider.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/widgets/map/layers/tiles/tile_registry/tile_provider.dart';

class OSMProvider extends TileProviderWrapper {
  final String? cachePath;

  OSMProvider({required this.cachePath});

  @override String get id => 'osm';
  @override String name(BuildContext context) => context.l10n.layerNameOsm;
  @override String description(BuildContext context) => context.l10n.layerDescriptionOsm;
  @override String get attributions => 'OpenStreetMap contributors';
  @override TileCategory get category => TileCategory.global;

  @override
  TileLayer createTileLayer() => TileLayer(
    urlTemplate: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
    tileProvider: cachePath != null
        ? CachedTileProvider(
      maxStale: const Duration(days: 30),
      store: HiveCacheStore(
        cachePath,
        hiveBoxName: 'osm_cache',
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