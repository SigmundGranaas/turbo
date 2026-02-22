import 'dart:async';
import 'dart:ui' as ui;
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';
import 'package:turbo/features/tile_storage/tile_store/api.dart';
import 'package:turbo/features/tile_storage/tile_store/utils/tile_provider_id_sanitizer.dart';

// Internal provider for a singleton Dio instance. Can be overridden in tests.
final dioProvider = Provider<Dio>((ref) {
  final dio = Dio();
  dio.options.connectTimeout = const Duration(seconds: 10);
  dio.options.receiveTimeout = const Duration(seconds: 10);
  dio.options.headers = {
    'User-Agent': 'turbo_map_app/1.0.18 (+https://github.com/sigmundgranaas/turbo)',
    'Accept': 'image/png,image/*;q=0.8,*/*;q=0.5',
    'Connection': 'keep-alive',
  };
  return dio;
});

// Internal provider for the CacheService. The public API will use this.
final cacheServiceProvider = FutureProvider<CacheService>((ref) async {
  // Await the tileStoreProvider to get the concrete instance
  final tileStore = await ref.watch(tileStoreServiceProvider.future);
  return CacheService(
    tileStore: tileStore,
    dio: ref.watch(dioProvider),
  );
});

/// A custom exception to signal that a tile was not found in local storage.
/// This is used to trigger flutter_map's error handling for over-zooming.
class _TileNotFoundInStore implements Exception {
  final String message;
  const _TileNotFoundInStore(this.message);
  @override
  String toString() => message;
}

/// A service to manage the lifecycle of cached tiles. It orchestrates
/// the interaction between the network and the TileStore.
class CacheService {
  final TileStoreService tileStore;
  final Dio dio;
  final _log = Logger('CacheService');

  // Manages in-flight requests to prevent duplicate network calls for the same tile.
  final Map<String, Future<Uint8List?>> _activeRequests = {};

  CacheService({required this.tileStore, required this.dio});

  TileProvider createTileProvider({
    required String urlTemplate,
    Map<String, String>? headers,
    String? userAgentPackageName,
  }) {
    return _CachingTileProvider(
      urlTemplate: urlTemplate,
      headers: headers,
      userAgentPackageName: userAgentPackageName,
      cacheService: this,
    );
  }

  /// The core logic for getting a tile's image bytes.
  /// It checks the cache first, then falls back to the network.
  Future<Uint8List?> getTileBytes(String providerId, TileCoordinates coords,
      String url, Map<String, String>? headers) {
    final key = '$providerId/${coords.z}/${coords.x}/${coords.y}';

    // If a request for this tile is already active, return the existing future.
    if (_activeRequests.containsKey(key)) {
      return _activeRequests[key]!;
    }

    // Otherwise, create a new future to represent the work to be done.
    final future = _fetchAndCacheTile(providerId, coords, url, headers, key);
    _activeRequests[key] = future;

    // When the future completes (or fails), remove it from the active map.
    future.whenComplete(() => _activeRequests.remove(key));

    return future;
  }

  Future<Uint8List?> _fetchAndCacheTile(String providerId,
      TileCoordinates coords, String url, Map<String, String>? headers, String key) async {
    // 1. Check the store (which now checks L1 and L2 caches transparently)
    final fromStore = await tileStore.get(providerId, coords);
    if (fromStore != null) {
      return fromStore;
    }

    // 2. Network fetch (cache miss)
    try {
      final response = await dio.get<Uint8List>(
        url,
        options: Options(responseType: ResponseType.bytes, headers: headers),
      );

      if (response.statusCode == 200 && response.data != null) {
        final bytes = response.data!;
        // 3. Put the new tile into the store. The store handles L1/L2 and eviction.
        // We don't await this, allowing the UI to render immediately.
        tileStore.put(providerId, coords, bytes);
        return bytes;
      } else {
        // Throw a specific exception type that includes response details.
        throw DioException(
          requestOptions: response.requestOptions,
          response: response,
          message: 'Network tile request failed with status: ${response.statusCode}',
        );
      }
    } on DioException catch (e) {
      _log.warning('Failed to fetch tile $key. Error: ${e.message}');
      // Return null instead of rethrowing to allow FutureImage to handle it gracefully
      // without polluting the global error handler.
      return null;
    } catch (e, s) {
      _log.severe('An unexpected error occurred fetching tile $key', e, s);
      return null;
    }
  }

  Future<int> clear() {
    _activeRequests.clear();
    tileStore.clearMemoryCache();
    return tileStore.clearDiskCache();
  }
}

/// Private TileProvider implementation that delegates to the CacheService.
class _CachingTileProvider extends TileProvider {
  final String urlTemplate;
  final String? userAgentPackageName;
  final CacheService cacheService;
  final String providerId;

  _CachingTileProvider({
    required this.urlTemplate,
    required this.cacheService,
    this.userAgentPackageName,
    super.headers,
  }) : providerId = sanitizeProviderId(urlTemplate);

  @override
  ImageProvider getImage(TileCoordinates coordinates, TileLayer options) {
    final url = getTileUrl(coordinates, options);
    final futureBytes =
    cacheService.getTileBytes(providerId, coordinates, url, headers);
    return FutureImage(futureBytes);
  }
}

/// An efficient ImageProvider that resolves a `Future<Uint8List?>`.
/// It directly decodes the bytes, avoiding intermediate Image objects.
class FutureImage extends ImageProvider<FutureImage> {
  final Future<Uint8List?> futureBytes;

  const FutureImage(this.futureBytes);

  @override
  Future<FutureImage> obtainKey(ImageConfiguration configuration) {
    return SynchronousFuture<FutureImage>(this);
  }

  @override
  ImageStreamCompleter loadImage(FutureImage key, ImageDecoderCallback decode) {
    return MultiFrameImageStreamCompleter(
      codec: _loadAsync(key, decode),
      scale: 1.0,
      informationCollector: () => <DiagnosticsNode>[
        DiagnosticsProperty<FutureImage>('Image provider', this),
        DiagnosticsProperty<Future<Uint8List?>>('Future', futureBytes),
      ],
    );
  }

  Future<ui.Codec> _loadAsync(
      FutureImage key, ImageDecoderCallback decode) async {
    try {
      final bytes = await key.futureBytes;
      if (bytes == null) {
        // This is an expected condition for missing offline tiles or when over-zooming.
        // We throw to trigger flutter_map's errorBuilder, which handles
        // rendering a parent tile. The exception is caught and handled by
        // flutter_map, but may still appear in debug logs with a more descriptive message.
        throw const _TileNotFoundInStore(
            'Tile not found in local store or cache.');
      }
      final buffer = await ui.ImmutableBuffer.fromUint8List(bytes);
      return decode(buffer);
    } catch (e) {
      // Any error resulting in null bytes (like Dio failures) should trigger
      // the errorBuilder or tile fallback in flutter_map.
      throw const _TileNotFoundInStore(
          'Tile fetch failed or not found.');
    }
  }

  @override
  bool operator ==(Object other) {
    if (other.runtimeType != runtimeType) return false;
    return other is FutureImage && other.futureBytes == futureBytes;
  }

  @override
  int get hashCode => futureBytes.hashCode;
}