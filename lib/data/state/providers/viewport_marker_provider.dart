import 'dart:async';
import 'package:flutter_map/flutter_map.dart' as fm;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/data/auth/auth_providers.dart';
import 'package:turbo/data/datastore/api_location_service.dart';
import 'package:turbo/data/model/marker.dart';
import 'package:turbo/data/datastore/marker_data_store.dart';
import 'location_repository.dart';

String _boundsToCacheKey(fm.LatLngBounds bounds, double zoom) {
  const precision = 5; // Adjust precision as needed for cache granularity
  return '${bounds.southWest.latitude.toStringAsFixed(precision)}_${bounds.southWest.longitude.toStringAsFixed(precision)}_${bounds.northEast.latitude.toStringAsFixed(precision)}_${bounds.northEast.longitude.toStringAsFixed(precision)}_${zoom.round()}';
}

final _viewportCache = <String, List<Marker>>{};
final _viewportCacheTimestamps = <String, DateTime>{};
const _cacheDuration = Duration(seconds: 30); // Shorter cache for viewport data

final viewportMarkerNotifierProvider = StateNotifierProvider.autoDispose<ViewportMarkerNotifier, AsyncValue<List<Marker>>>((ref) {
  return ViewportMarkerNotifier(ref);
});

class ViewportMarkerNotifier extends StateNotifier<AsyncValue<List<Marker>>> {
  final Ref _ref;
  Timer? _debounceTimer;

  ViewportMarkerNotifier(this._ref) : super(const AsyncValue.data([]));

  ApiLocationService get _apiService => _ref.read(apiLocationServiceProvider);
  AuthStatus get _authStatus => _ref.watch(authStateProvider).status;

  void loadMarkersInViewport(fm.LatLngBounds bounds, double currentZoom) {
    _debounceTimer?.cancel();
    _debounceTimer = Timer(const Duration(milliseconds: 500), () async {
      if (!mounted) return;

      // Do not set global loading if current data is available, for smoother UX
      // Only set to loading if truly fetching for the first time or after error.
      if (state is! AsyncData || (state as AsyncData).value.isEmpty) {
        state = const AsyncValue.loading();
      }

      final querySW = LatLng(bounds.southWest.latitude, bounds.southWest.longitude);
      final queryNE = LatLng(bounds.northEast.latitude, bounds.northEast.longitude);

      final cacheKey = _boundsToCacheKey(bounds, currentZoom);
      final cachedTime = _viewportCacheTimestamps[cacheKey];

      if (cachedTime != null && DateTime.now().difference(cachedTime) < _cacheDuration) {
        final cachedMarkers = _viewportCache[cacheKey];
        if (cachedMarkers != null) {
          if (mounted) state = AsyncValue.data(cachedMarkers);
          return;
        }
      }

      try {
        List<Marker> markers;
        if (_authStatus == AuthStatus.authenticated) {
          markers = await _apiService.getLocationsInExtent(querySW, queryNE);
        } else {
          final localStore = await _ref.read(localMarkerDataStoreProvider.future);
          markers = await localStore.findInBounds(querySW, queryNE);
        }
        _viewportCache[cacheKey] = markers;
        _viewportCacheTimestamps[cacheKey] = DateTime.now();
        if (mounted) state = AsyncValue.data(markers);
      } catch (e, st) {
        if (mounted) state = AsyncValue.error(e, st);
      }
    });
  }

  void clearViewportCacheAndReload(fm.LatLngBounds bounds, double currentZoom) {
    _viewportCache.remove(_boundsToCacheKey(bounds, currentZoom));
    _viewportCacheTimestamps.remove(_boundsToCacheKey(bounds, currentZoom));
    loadMarkersInViewport(bounds, currentZoom);
  }

  void invalidateCache() {
    _viewportCache.clear();
    _viewportCacheTimestamps.clear();
  }


  @override
  void dispose() {
    _debounceTimer?.cancel();
    super.dispose();
  }
}