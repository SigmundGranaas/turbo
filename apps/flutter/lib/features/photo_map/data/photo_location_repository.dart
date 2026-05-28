import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:logging/logging.dart';
// Hide photo_manager's `LatLng` in favour of latlong2's.
import 'package:photo_manager/photo_manager.dart' hide LatLng;

import '../models/photo_location.dart';

final _log = Logger('PhotoLocations');

/// Lifecycle of the device photo-library scan that backs the photo map
/// layer. [unsupported] is a terminal state for platforms without a photo
/// library (web/desktop); [permissionDenied] is recoverable — the user can
/// grant access and we reload.
enum PhotoLibraryStatus {
  idle,
  loading,
  ready,
  permissionDenied,
  unsupported,
  error,
}

@immutable
class PhotoLocationState {
  final PhotoLibraryStatus status;
  final List<PhotoLocation> photos;
  final Object? error;

  const PhotoLocationState({
    this.status = PhotoLibraryStatus.idle,
    this.photos = const [],
    this.error,
  });

  bool get isReady => status == PhotoLibraryStatus.ready;

  PhotoLocationState copyWith({
    PhotoLibraryStatus? status,
    List<PhotoLocation>? photos,
    Object? error,
  }) {
    return PhotoLocationState(
      status: status ?? this.status,
      photos: photos ?? this.photos,
      error: error,
    );
  }
}

/// Scans the device photo library for geotagged images and exposes them as
/// [PhotoLocation]s. The scan is lazy: it only runs once [ensureLoaded] is
/// called (when the user first turns the layer on), so the OS photo-access
/// prompt never appears unprompted.
final photoLocationRepositoryProvider =
    NotifierProvider<PhotoLocationNotifier, PhotoLocationState>(
  PhotoLocationNotifier.new,
);

class PhotoLocationNotifier extends Notifier<PhotoLocationState> {
  bool _loading = false;

  @override
  PhotoLocationState build() => const PhotoLocationState();

  /// photo_manager only ships native implementations for these platforms.
  /// `defaultTargetPlatform` is web-safe (unlike `dart:io`'s `Platform`).
  bool get _isSupported {
    if (kIsWeb) return false;
    switch (defaultTargetPlatform) {
      case TargetPlatform.android:
      case TargetPlatform.iOS:
      case TargetPlatform.macOS:
        return true;
      default:
        return false;
    }
  }

  /// Load once. Repeated calls while ready/loading are no-ops, so this is
  /// safe to call from `build()` of the layer widget on every rebuild.
  Future<void> ensureLoaded() async {
    if (state.status == PhotoLibraryStatus.ready ||
        state.status == PhotoLibraryStatus.loading) {
      return;
    }
    await load();
  }

  /// Force a fresh scan, e.g. after the user grants access from settings.
  Future<void> refresh() async {
    state = const PhotoLocationState();
    await load();
  }

  Future<void> load() async {
    if (_loading) return;
    if (!_isSupported) {
      state = const PhotoLocationState(status: PhotoLibraryStatus.unsupported);
      return;
    }
    _loading = true;
    state = state.copyWith(status: PhotoLibraryStatus.loading);
    try {
      final permission = await PhotoManager.requestPermissionExtend();
      if (!permission.hasAccess) {
        state =
            const PhotoLocationState(status: PhotoLibraryStatus.permissionDenied);
        return;
      }
      final photos = await _fetchGeotaggedPhotos();
      _log.info('Loaded ${photos.length} geotagged photos');
      state = PhotoLocationState(
        status: PhotoLibraryStatus.ready,
        photos: photos,
      );
    } catch (e, st) {
      _log.warning('Failed to load photo locations', e, st);
      state = PhotoLocationState(status: PhotoLibraryStatus.error, error: e);
    } finally {
      _loading = false;
    }
  }

  Future<List<PhotoLocation>> _fetchGeotaggedPhotos() async {
    // `onlyAll` collapses the library into a single virtual album so we
    // page through every image exactly once instead of per-album.
    final paths = await PhotoManager.getAssetPathList(
      type: RequestType.image,
      onlyAll: true,
    );
    if (paths.isEmpty) return const [];

    final album = paths.first;
    final total = await album.assetCountAsync;
    if (total == 0) return const [];

    final result = <PhotoLocation>[];
    const pageSize = 1000;
    final pageCount = (total / pageSize).ceil();

    for (var page = 0; page < pageCount; page++) {
      final assets = await album.getAssetListPaged(page: page, size: pageSize);
      for (final asset in assets) {
        // The lat/long carried on the asset come straight from MediaStore
        // (Android) / PHAsset.location (iOS) — cheap, no per-file EXIF read.
        // Photos without a fix report null or (0, 0); skip those.
        final lat = asset.latitude;
        final lng = asset.longitude;
        if (lat == null || lng == null) continue;
        if (lat == 0 && lng == 0) continue;
        result.add(PhotoLocation(
          id: asset.id,
          position: LatLng(lat, lng),
          asset: asset,
          createdAt: asset.createDateTime,
        ));
      }
    }
    return result;
  }
}
