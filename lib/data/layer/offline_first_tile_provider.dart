import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_cancellable_tile_provider/flutter_map_cancellable_tile_provider.dart';
import 'package:path/path.dart' as p;

class OfflineFirstTileProvider extends TileProvider {
  final String tileCacheBasePath;
  final TileProvider _networkTileProvider;
  final bool offlineOnly;

  OfflineFirstTileProvider({
    required this.tileCacheBasePath,
    Map<String, String>? headers,
    this.offlineOnly = false,
  }) : _networkTileProvider = CancellableNetworkTileProvider(headers: headers);

  @override
  bool get supportsCancelLoading => true;

  @override
  ImageProvider getImageWithCancelLoadingSupport(
      TileCoordinates coordinates,
      TileLayer options,
      Future<void> cancelLoading,
      ) {
    final sanitisedUrl = options.urlTemplate
        ?.replaceAll(RegExp(r'https?://'), '')
        .replaceAll(RegExp(r'[^a-zA-Z0-9]'), '_');

    final tilePath = p.join(
      tileCacheBasePath,
      sanitisedUrl,
      coordinates.z.toString(),
      coordinates.x.toString(),
      '${coordinates.y}.png',
    );

    final file = File(tilePath);

    if (file.existsSync()) {
      return FileImage(file);
    }

    if (offlineOnly) {
      throw Exception('Tile not found in offline-only mode.');
    }

    // Fallback to the network provider.
    return _networkTileProvider.getImageWithCancelLoadingSupport(
      coordinates,
      options,
      cancelLoading,
    );
  }

  @override
  ImageProvider getImage(TileCoordinates coordinates, TileLayer options) {
    throw UnimplementedError(
      'This TileProvider supports cancellation. Use `getImageWithCancelLoadingSupport` instead.',
    );
  }
}