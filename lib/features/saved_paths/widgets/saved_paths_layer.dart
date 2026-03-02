import 'dart:async';
import 'package:flutter/material.dart' hide CatmullRomSpline;
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/util/catmull_rom_spline.dart';
import 'package:turbo/features/markers/data/icon_service.dart';
import 'package:turbo/l10n/app_localizations.dart';
import '../data/data_visibility_provider.dart';
import '../data/saved_path_repository.dart';
import '../data/viewport_saved_path_provider.dart';
import '../models/path_style.dart';
import '../models/saved_path.dart';
import 'path_detail_sheet.dart';

class SavedPathsLayer extends ConsumerStatefulWidget {
  final MapController mapController;

  const SavedPathsLayer({super.key, required this.mapController});

  @override
  ConsumerState<SavedPathsLayer> createState() => _SavedPathsLayerState();
}

class _SavedPathsLayerState extends ConsumerState<SavedPathsLayer> {
  final LayerHitNotifier<String> _hitNotifier = ValueNotifier(null);
  StreamSubscription<MapEvent>? _mapEventSubscription;
  ProviderSubscription<AsyncValue<List<SavedPath>>>? _repoSubscription;
  bool _isMapReady = false;
  final IconService _iconService = IconService();

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
          if (!_isMapReady) {
            _isMapReady = true;
          }
          _updateViewportPaths();
        }
      });

      _repoSubscription =
          ref.listenManual(savedPathRepositoryProvider, (prev, next) {
        if (_isMapReady) {
          ref
              .read(viewportSavedPathNotifierProvider.notifier)
              .invalidateCache();
          _updateViewportPaths();
        }
      });
    });
  }

  @override
  void dispose() {
    _hitNotifier.dispose();
    _mapEventSubscription?.cancel();
    _repoSubscription?.close();
    super.dispose();
  }

  void _updateViewportPaths() {
    if (!mounted || !_isMapReady) return;
    final bounds = widget.mapController.camera.visibleBounds;
    ref
        .read(viewportSavedPathNotifierProvider.notifier)
        .loadPathsInViewport(bounds);
  }

  bool _shouldShowAsIcon(SavedPath path, MapCamera camera) {
    if (path.iconKey == null) return false;
    final b = path.bounds;
    final sw = camera.latLngToScreenOffset(b.southWest);
    final ne = camera.latLngToScreenOffset(b.northEast);
    return (ne.dx - sw.dx).abs() < 40 && (ne.dy - sw.dy).abs() < 40;
  }

  @override
  Widget build(BuildContext context) {
    final isVisible = ref.watch(savedPathsVisibleProvider);
    if (!isVisible) return const SizedBox.shrink();

    final pathsAsync = ref.watch(viewportSavedPathNotifierProvider);
    final colorScheme = Theme.of(context).colorScheme;

    return pathsAsync.when(
      data: (paths) {
        if (paths.isEmpty) return const SizedBox.shrink();
        return _buildLayer(paths, colorScheme);
      },
      loading: () {
        final previousData =
            ref.read(viewportSavedPathNotifierProvider).asData?.value;
        if (previousData != null && previousData.isNotEmpty) {
          return _buildLayer(previousData, colorScheme);
        }
        return const SizedBox.shrink();
      },
      error: (_, _) => const SizedBox.shrink(),
    );
  }

  Widget _buildLayer(List<SavedPath> paths, ColorScheme colorScheme) {
    final camera = widget.mapController.camera;
    final polylinePaths = <SavedPath>[];
    final iconPaths = <SavedPath>[];

    for (final path in paths) {
      if (_shouldShowAsIcon(path, camera)) {
        iconPaths.add(path);
      } else {
        polylinePaths.add(path);
      }
    }

    return GestureDetector(
      onTap: () {
        final hit = _hitNotifier.value;
        if (hit != null && hit.hitValues.isNotEmpty) {
          final uuid = hit.hitValues.first;
          final path = paths.cast<SavedPath?>().firstWhere(
                (p) => p!.uuid == uuid,
                orElse: () => null,
              );
          if (path != null) {
            _showDetailSheet(context, path);
          }
        }
      },
      child: Stack(
        children: [
          PolylineLayer(
            hitNotifier: _hitNotifier,
            polylines: polylinePaths.map((path) {
              final points = path.smoothing
                  ? CatmullRomSpline(controlPoints: path.points).generate()
                  : path.points;
              final color =
                  (hexToColor(path.colorHex) ?? colorScheme.primary)
                      .withAlpha(180);
              final pattern =
                  PathLineStyle.fromKey(path.lineStyleKey).toStrokePattern();

              return Polyline(
                points: points,
                strokeWidth: 5.5,
                color: color,
                strokeCap: StrokeCap.round,
                strokeJoin: StrokeJoin.round,
                pattern: pattern,
                hitValue: path.uuid,
              );
            }).toList(),
          ),
          if (iconPaths.isNotEmpty)
            MarkerLayer(
              markers: iconPaths.map((path) {
                final center = path.bounds.center;
                final namedIcon =
                    _iconService.getIcon(context, path.iconKey);
                final color =
                    hexToColor(path.colorHex) ?? colorScheme.primary;

                return Marker(
                  point: center,
                  width: 40,
                  height: 40,
                  child: GestureDetector(
                    onTap: () => _showDetailSheet(context, path),
                    child: Icon(
                      namedIcon.icon,
                      color: color,
                      size: 32,
                    ),
                  ),
                );
              }).toList(),
            ),
        ],
      ),
    );
  }

  void _showDetailSheet(BuildContext context, SavedPath path) async {
    final l10n = context.l10n;
    final messenger = ScaffoldMessenger.of(context);
    final result = await showModalBottomSheet<PathDetailResult>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (context) => PathDetailSheet(path: path),
    );
    if (!mounted) return;
    if (result == PathDetailResult.updated) {
      messenger.showSnackBar(
        SnackBar(
          content: Text(l10n.pathUpdated),
          behavior: SnackBarBehavior.floating,
          shape:
              RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
          margin: const EdgeInsets.all(16),
        ),
      );
    } else if (result == PathDetailResult.deleted) {
      messenger.showSnackBar(
        SnackBar(
          content: Text(l10n.pathDeleted),
          behavior: SnackBarBehavior.floating,
          shape:
              RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
          margin: const EdgeInsets.all(16),
        ),
      );
    }
  }
}
