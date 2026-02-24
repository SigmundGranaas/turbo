import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/tile_storage/cached_tiles/data/cache_service.dart';

export 'package:turbo/features/tile_storage/cached_tiles/data/cache_service.dart' show FutureImage;

/// Public API for the Cached Tiles feature.
///
/// This feature provides a [TileProvider] that orchestrates
/// network fetching and caching via the `TileStoreService`.
final cacheApiProvider = FutureProvider<CacheApi>((ref) async {
  // Await the async cache service provider
  final cacheService = await ref.watch(cacheServiceProvider.future);
  return CacheApi(cacheService: cacheService);
});


class CacheApi {
  final CacheService _cacheService;
  CacheApi({required CacheService cacheService}) : _cacheService = cacheService;

  /// Creates a highly performant TileProvider that automatically caches tiles
  /// to a shared, performance-optimized storage layer.
  ///
  /// This is the primary method to be used by UI layers when creating a
  /// standard, network-backed map layer.
  TileProvider createTileProvider({
    required String urlTemplate,
    Map<String, String>? headers,
    String? userAgentPackageName,
  }) {
    return _cacheService.createTileProvider(
      urlTemplate: urlTemplate,
      headers: headers,
      userAgentPackageName: userAgentPackageName,
    );
  }

  /// Deletes all cached tiles that are not part of a downloaded offline region.
  Future<int> clearCache() {
    return _cacheService.clear();
  }
}