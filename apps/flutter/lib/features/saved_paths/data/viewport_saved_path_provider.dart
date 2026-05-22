import 'dart:async';
import 'package:flutter_map/flutter_map.dart' as fm;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'saved_path_repository.dart';
import '../models/saved_path.dart';

String _boundsToCacheKey(fm.LatLngBounds bounds) {
  const precision = 5;
  return '${bounds.southWest.latitude.toStringAsFixed(precision)}_${bounds.southWest.longitude.toStringAsFixed(precision)}_${bounds.northEast.latitude.toStringAsFixed(precision)}_${bounds.northEast.longitude.toStringAsFixed(precision)}';
}

final _viewportCache = <String, List<SavedPath>>{};
final _viewportCacheTimestamps = <String, DateTime>{};
const _cacheDuration = Duration(seconds: 30);

final viewportSavedPathNotifierProvider =
    NotifierProvider.autoDispose<ViewportSavedPathNotifier, AsyncValue<List<SavedPath>>>(() {
  return ViewportSavedPathNotifier();
});

class ViewportSavedPathNotifier extends Notifier<AsyncValue<List<SavedPath>>> {
  Timer? _debounceTimer;

  @override
  AsyncValue<List<SavedPath>> build() {
    ref.onDispose(() {
      _debounceTimer?.cancel();
    });
    return const AsyncValue.data([]);
  }

  void loadPathsInViewport(fm.LatLngBounds bounds) {
    _debounceTimer?.cancel();
    _debounceTimer = Timer(const Duration(milliseconds: 500), () async {
      if (state is! AsyncData || (state as AsyncData).value.isEmpty) {
        state = const AsyncValue.loading();
      }

      final querySW = LatLng(bounds.southWest.latitude, bounds.southWest.longitude);
      final queryNE = LatLng(bounds.northEast.latitude, bounds.northEast.longitude);

      final cacheKey = _boundsToCacheKey(bounds);
      final cachedTime = _viewportCacheTimestamps[cacheKey];

      if (cachedTime != null && DateTime.now().difference(cachedTime) < _cacheDuration) {
        final cachedPaths = _viewportCache[cacheKey];
        if (cachedPaths != null) {
          state = AsyncValue.data(cachedPaths);
          return;
        }
      }

      try {
        final localStore = await ref.read(localSavedPathDataStoreProvider.future);
        final paths = await localStore.findInBounds(querySW, queryNE);
        _viewportCache[cacheKey] = paths;
        _viewportCacheTimestamps[cacheKey] = DateTime.now();
        state = AsyncValue.data(paths);
      } catch (e, st) {
        state = AsyncValue.error(e, st);
      }
    });
  }

  void invalidateCache() {
    _viewportCache.clear();
    _viewportCacheTimestamps.clear();
  }
}
