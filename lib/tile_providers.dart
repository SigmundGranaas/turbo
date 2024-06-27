
import 'package:flutter/cupertino.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_cancellable_tile_provider/flutter_map_cancellable_tile_provider.dart';
import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'dart:async';


TileLayer get openStreetMapTileLayer => TileLayer(
  urlTemplate: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
  // Use the recommended flutter_map_cancellable_tile_provider package to
  // support the cancellation of loading tiles.
  tileProvider: CancellableNetworkTileProvider(),
);

TileLayer get norgesKart => TileLayer(
  urlTemplate: 'https://cache.kartverket.no/topo/v1/wmts/1.0.0/default/googlemaps/{z}/{y}/{x}.png',
  // Use the recommended flutter_map_cancellable_tile_provider package to
  // support the cancellation of loading tiles.
  tileProvider: CancellableNetworkTileProvider(),
);

TileLayer norgesKartSatelitt(String gkt) => TileLayer(
  tileProvider: CustomNorwayTileProvider(gkt),
);


class CustomNorwayTileProvider extends TileProvider {
  final String baseUrl = 'https://gatekeeper1.geonorge.no/BaatGatekeeper/gk/gk.nib_web_mercator_wmts_v2';

  final String gkt;

  CustomNorwayTileProvider(this.gkt);


  @override
  ImageProvider getImage(TileCoordinates coordinates, TileLayer options) {
    final url = '$baseUrl?'
        'layer=Nibcache_web_mercator_v2'
        '&gkt=$gkt'
        '&style=default'
        '&tilematrixset=GoogleMapsCompatible'
        '&Service=WMTS'
        '&Request=GetTile'
        '&Version=1.0.0'
        '&Format=image/png'
        '&TileMatrix=${coordinates.z}'
        '&TileCol=${coordinates.x}'
        '&TileRow=${coordinates.y}';
    return NetworkImage(url, headers: {'User-Agent': 'app/1.0'});
  }
}

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
