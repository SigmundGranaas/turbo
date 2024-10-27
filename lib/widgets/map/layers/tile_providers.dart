
import 'package:dio_cache_interceptor_hive_store/dio_cache_interceptor_hive_store.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_cache/flutter_map_cache.dart';
import 'package:flutter_map_cancellable_tile_provider/flutter_map_cancellable_tile_provider.dart';
import 'package:http/http.dart' as http;
import 'package:path_provider/path_provider.dart';
import 'dart:async';


Future<String?> getPath() async {
  if (kIsWeb) {
    return Future.value(null);
  } else {
    final cacheDirectory = await getTemporaryDirectory();
    return cacheDirectory.path;
  }

}

TileLayer openStreetMap(path) => TileLayer(
  urlTemplate: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
  // Use the recommended flutter_map_cancellable_tile_provider package to
  // support the cancellation of loading tiles.
  tileProvider: CancellableNetworkTileProvider()
);

TileLayer googleSatellite(path) => TileLayer(
    urlTemplate: 'https://mt0.google.com/vt/lyrs=s&hl=en&x={x}&y={y}&z={z}',
    // Use the recommended flutter_map_cancellable_tile_provider package to
    // support the cancellation of loading tiles.
    tileProvider: CancellableNetworkTileProvider()
);

TileLayer norgesKart(path) => TileLayer(
  urlTemplate: 'https://cache.kartverket.no/topo/v1/wmts/1.0.0/default/googlemaps/{z}/{y}/{x}.png',
  tileProvider: CachedTileProvider(
    // maxStale keeps the tile cached for the given Duration and
    // tries to revalidate the next time it gets requested
    maxStale: const Duration(days: 30),
    store: HiveCacheStore(
      path,
      hiveBoxName: 'HiveCacheStore',
    ),
  ),
);


const String baseUrl = 'https://gatekeeper1.geonorge.no/BaatGatekeeper/gk/gk.nib_web_mercator_wmts_v2';
 url (gkt) => '$baseUrl?'
    'layer=Nibcache_web_mercator_v2'
    '&gkt=$gkt'
    '&style=default'
    '&tilematrixset=GoogleMapsCompatible'
    '&Service=WMTS'
    '&Request=GetTile'
    '&Version=1.0.0'
    '&Format=image/png'
    '&TileMatrix={z}'
    '&TileCol={x}'
    '&TileRow={y}';

TileLayer norgeSatelitt((String, String?) input) => TileLayer(
  urlTemplate: url(input.$1),
  tileProvider: CachedTileProvider(
    // maxStale keeps the tile cached for the given Duration and
    // tries to revalidate the next time it gets requested
    maxStale: const Duration(days: 30),
    store: HiveCacheStore(
      input.$2,
      hiveBoxName: 'HiveCacheStore',
    ),
  ),
);

class GktManager {
  static final GktManager _instance = GktManager._internal();
  factory GktManager() => _instance;
  GktManager._internal();

  String? _gkt;
  DateTime? _lastFetchTime;

  Future<String> getGkt() async {
    if (_gkt != null && _lastFetchTime != null &&
        DateTime.now().difference(_lastFetchTime!).inHours < 1) {
      return _gkt!;
    }

    try {
      final response = await http.get(
        Uri.parse('https://norgeskart.no/ws/gatekeeper.py?key=73e029c3632c49bb1586fc57a60fb701kv'),
        headers: {'User-Agent': 'app/1.0'},
      ).timeout(const Duration(seconds: 10));

      if (response.statusCode == 200) {
        _gkt = response.body.trim().replaceAll('"', '');
        _lastFetchTime = DateTime.now();
        return _gkt!;
      } else {
        throw Exception('Failed to load GKT code');
      }
    } catch (e) {
      throw Exception('Error fetching GKT code: $e');
    }
  }
}
