import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:turbo/features/tile_storage/cached_tiles/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/models/offline_region.dart';
import 'package:turbo/features/tile_storage/tile_store/api.dart';
import '../../tile_store/utils/tile_provider_id_sanitizer.dart';

/// A TileProvider that ONLY loads tiles from the local TileStore for a specific
/// [OfflineRegion]. It does not fall back to the network.
class OfflineTileProvider extends TileProvider {
  final OfflineRegion region;
  final TileStoreService tileStore;
  final String providerId;

  OfflineTileProvider({
    required this.region,
    required this.tileStore,
  }) : providerId = sanitizeProviderId(region.urlTemplate);

  @override
  ImageProvider getImage(TileCoordinates coordinates, TileLayer options) {
    return FutureImage(
      tileStore.get(providerId, coordinates),
    );
  }
}

// A 1x1 transparent pixel PNG.
final kTransparentImage = Uint8List.fromList([
  137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0,
  0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120,
  218, 99, 96, 0, 0, 0, 6, 0, 2, 124, 1, 166, 89, 0, 0, 0, 0, 73, 69, 78, 68,
  174, 66, 96, 130
]);