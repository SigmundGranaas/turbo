import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:photo_manager/photo_manager.dart' hide LatLng;
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/core/widgets/map/controller/map_utility.dart';

import '../data/photo_layer_visibility_provider.dart';
import '../data/photo_location_repository.dart';
import '../models/photo_location.dart';
import 'photo_thumbnail.dart';
import 'photo_viewer.dart';

/// Renders geotagged device photos on the map as grid-clustered markers.
///
/// Clustering is done in Web-Mercator pixel space at the current zoom: every
/// in-view photo is bucketed into a fixed-size pixel grid, so nearby photos
/// collapse into one badge that naturally splits apart as you zoom in. This
/// keeps the layer independent of flutter_map's internal projection API and
/// cheap to recompute on each pan/zoom.
class PhotoMapLayer extends ConsumerStatefulWidget {
  final MapController mapController;

  const PhotoMapLayer({super.key, required this.mapController});

  @override
  ConsumerState<PhotoMapLayer> createState() => _PhotoMapLayerState();
}

class _PhotoMapLayerState extends ConsumerState<PhotoMapLayer>
    with TickerProviderStateMixin {
  // Pixel size of each clustering cell. Photos whose projected positions fall
  // in the same cell merge into a single marker.
  static const double _clusterCellPx = 72.0;
  // Below this zoom a cluster tap zooms in to split it; at/above it a tap
  // opens the grid sheet because zooming further won't separate the photos.
  static const double _expandZoomCeiling = 16.0;

  StreamSubscription<MapEvent>? _mapEventSubscription;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _mapEventSubscription =
          widget.mapController.mapEventStream.listen((event) {
        if (event is MapEventMoveEnd ||
            event is MapEventRotateEnd ||
            event is MapEventFlingAnimationEnd ||
            event is MapEventDoubleTapZoomEnd ||
            event is MapEventScrollWheelZoom) {
          // Re-cluster against the new camera. Cheap and keyed off cached
          // thumbnails, so a plain rebuild is fine.
          if (mounted) setState(() {});
        }
      });
    });
  }

  @override
  void dispose() {
    _mapEventSubscription?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final isVisible = ref.watch(photoLayerVisibleProvider);

    // Surface terminal/permission outcomes to the user once, the first time
    // they occur while the layer is on.
    ref.listen<PhotoLocationState>(photoLocationRepositoryProvider,
        (prev, next) {
      if (!ref.read(photoLayerVisibleProvider)) return;
      if (prev?.status == next.status) return;
      if (next.status == PhotoLibraryStatus.permissionDenied) {
        AppSnackbars.info(context, context.l10n.photoPermissionRequired);
      } else if (next.status == PhotoLibraryStatus.unsupported) {
        AppSnackbars.info(context, context.l10n.photosNotAvailableHere);
      }
    });

    if (!isVisible) return const SizedBox.shrink();

    final libraryState = ref.watch(photoLocationRepositoryProvider);

    // Kick off the (one-time) scan the first time the layer is shown. Done
    // off the build phase to avoid mutating providers mid-build.
    if (libraryState.status == PhotoLibraryStatus.idle) {
      Future.microtask(
        () => ref.read(photoLocationRepositoryProvider.notifier).ensureLoaded(),
      );
      return const SizedBox.shrink();
    }

    if (libraryState.photos.isEmpty) return const SizedBox.shrink();

    final clusters = _buildClusters(libraryState.photos);
    return MarkerLayer(
      markers: [for (final cluster in clusters) _buildMarker(cluster)],
    );
  }

  List<_PhotoCluster> _buildClusters(List<PhotoLocation> photos) {
    final camera = widget.mapController.camera;
    final zoom = camera.zoom;
    final bounds = camera.visibleBounds;
    final buckets = <String, _PhotoCluster>{};

    for (final photo in photos) {
      if (!bounds.contains(photo.position)) continue;
      final px = _pixelX(photo.position.longitude, zoom);
      final py = _pixelY(photo.position.latitude, zoom);
      final key = '${(px / _clusterCellPx).floor()}:'
          '${(py / _clusterCellPx).floor()}';
      buckets.putIfAbsent(key, () => _PhotoCluster()).add(photo);
    }
    return buckets.values.toList(growable: false);
  }

  Marker _buildMarker(_PhotoCluster cluster) {
    final representative = cluster.representative;
    final isCluster = cluster.photos.length > 1;
    final size = isCluster ? 60.0 : 56.0;

    return Marker(
      point: representative.position,
      width: size,
      height: size,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () => _onMarkerTap(cluster),
        child: _PhotoMarkerBadge(
          asset: representative.asset,
          size: size,
          count: isCluster ? cluster.photos.length : null,
        ),
      ),
    );
  }

  void _onMarkerTap(_PhotoCluster cluster) {
    if (cluster.photos.length == 1) {
      showPhotoViewer(context, cluster.photos.first);
      return;
    }
    final camera = widget.mapController.camera;
    if (camera.zoom < _expandZoomCeiling) {
      // Zoom toward the cluster so it breaks apart into smaller groups.
      animatedMapMove(
        cluster.representative.position,
        math.min(camera.zoom + 2, _expandZoomCeiling),
        widget.mapController,
        this,
      );
    } else {
      showPhotoClusterSheet(context, cluster.photos);
    }
  }

  // --- Web Mercator pixel projection (independent of flutter_map internals).
  double _pixelX(double lon, double zoom) {
    final n = 256.0 * math.pow(2.0, zoom);
    return (lon + 180.0) / 360.0 * n;
  }

  double _pixelY(double lat, double zoom) {
    final n = 256.0 * math.pow(2.0, zoom);
    final latRad = lat * math.pi / 180.0;
    return (1.0 -
            (math.log(math.tan(latRad) + 1.0 / math.cos(latRad)) / math.pi)) /
        2.0 *
        n;
  }
}

/// A mutable accumulator for photos that fall into one grid cell. The
/// representative (shown as the marker's thumbnail) is the most recent photo.
class _PhotoCluster {
  final List<PhotoLocation> photos = [];

  void add(PhotoLocation photo) => photos.add(photo);

  PhotoLocation get representative {
    var best = photos.first;
    for (final p in photos) {
      final a = p.createdAt;
      final b = best.createdAt;
      if (a != null && (b == null || a.isAfter(b))) best = p;
    }
    return best;
  }
}

/// The on-map visual: a rounded thumbnail with a white frame and shadow,
/// plus a count badge when it represents more than one photo.
class _PhotoMarkerBadge extends StatelessWidget {
  final AssetEntity asset;
  final double size;
  final int? count;

  const _PhotoMarkerBadge({
    required this.asset,
    required this.size,
    this.count,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final thumb = Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: Colors.white, width: 2),
        boxShadow: const [
          BoxShadow(color: Colors.black26, blurRadius: 4, offset: Offset(0, 2)),
        ],
      ),
      clipBehavior: Clip.antiAlias,
      child: PhotoThumbnail(asset: asset, size: size),
    );

    if (count == null) return thumb;

    return Stack(
      clipBehavior: Clip.none,
      children: [
        thumb,
        Positioned(
          top: -6,
          right: -6,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
            constraints: const BoxConstraints(minWidth: 22, minHeight: 22),
            decoration: BoxDecoration(
              color: colorScheme.primary,
              shape: BoxShape.rectangle,
              borderRadius: BorderRadius.circular(11),
              border: Border.all(color: Colors.white, width: 1.5),
            ),
            alignment: Alignment.center,
            child: Text(
              count! > 99 ? '99+' : '$count',
              style: TextStyle(
                color: colorScheme.onPrimary,
                fontSize: 11,
                fontWeight: FontWeight.bold,
              ),
            ),
          ),
        ),
      ],
    );
  }
}
