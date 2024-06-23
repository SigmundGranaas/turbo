
import 'package:flutter/cupertino.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_cancellable_tile_provider/flutter_map_cancellable_tile_provider.dart';
import 'package:flutter/material.dart';
import 'package:proj4dart/proj4dart.dart' as proj4;


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

TileLayer get norgesKartSatelitt => TileLayer(
  tileProvider: CustomNorwayTileProvider(),
);


class CustomNorwayTileProvider extends TileProvider {
  final String baseUrl = 'https://gatekeeper1.geonorge.no/BaatGatekeeper/gk/gk.nib_web_mercator_wmts_v2';

  @override
  ImageProvider getImage(TileCoordinates coordinates, TileLayer options) {
    final url = '$baseUrl?'
        'layer=Nibcache_web_mercator_v2'
        '&gkt=318F5FCE232B94955C071B618FBA7DD1E891FAC3C07DBF000D7733B79E21E0BBC1AD0D4B4F2857DAE7C1F3CF9034009318B855F07FC2AB828D018F0853CD0DA1'
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