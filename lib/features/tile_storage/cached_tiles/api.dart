/// The public API for the Cached Tiles feature.
///
/// Provides a [CacheService] that orchestrates network fetching and caching
/// of map tiles. The service is the public surface — consumers do
/// `ref.read(cacheServiceProvider.future)` and call methods on the result.
library;

export 'data/cache_service.dart'
    show CacheService, cacheServiceProvider, FutureImage;
