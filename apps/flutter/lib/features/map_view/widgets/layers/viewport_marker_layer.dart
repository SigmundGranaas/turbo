import 'dart:async';
import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/core/widgets/map/map_marker_pin.dart';
import 'package:turbo/features/collections/api.dart';
import 'package:turbo/features/markers/api.dart' as marker_model;
import 'package:turbo/features/markers/api.dart' hide Marker;
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

final _log = Logger('ViewportMarkers');

class ViewportMarkers extends ConsumerStatefulWidget {
  final MapController mapController;

  const ViewportMarkers({
    super.key,
    required this.mapController,
  });

  @override
  ConsumerState<ViewportMarkers> createState() => _ViewportMarkersState();
}

class _ViewportMarkersState extends ConsumerState<ViewportMarkers> {
  StreamSubscription<MapEvent>? _mapEventSubscription;
  ProviderSubscription<AsyncValue<List<marker_model.Marker>>>?
  _locationRepositorySubscription;
  bool _isMapReady = false;

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
              _updateViewportMarkers();
            }
          });

      _locationRepositorySubscription =
          ref.listenManual(locationRepositoryProvider, (prev, next) {
            if (_isMapReady) {
              _updateViewportMarkers();
            }
          });
    });
  }

  @override
  void dispose() {
    _mapEventSubscription?.cancel();
    _locationRepositorySubscription?.close();
    super.dispose();
  }

  void _updateViewportMarkers() {
    if (!mounted || !_isMapReady) return;

    final bounds = widget.mapController.camera.visibleBounds;
    final zoom = widget.mapController.camera.zoom;
    ref
        .read(viewportMarkerNotifierProvider.notifier)
        .loadMarkersInViewport(bounds, zoom);
  }

  @override
  Widget build(BuildContext context) {
    final isVisible = ref.watch(markersVisibleProvider);
    if (!isVisible) return const SizedBox.shrink();

    final viewportMarkersAsync = ref.watch(viewportMarkerNotifierProvider);
    final selection = ref.watch(markerSelectionProvider);
    final iconService = IconService();
    final collectionState =
        ref.watch(collectionRepositoryProvider).asData?.value ??
            const CollectionRepositoryState.empty();
    final visibility = ref.watch(collectionVisibilityProvider);

    List<marker_model.Marker> applyFilter(List<marker_model.Marker> input) {
      if (collectionState.membershipIndex.isEmpty) return input;
      return input
          .where((m) => isItemVisibleForCollections(
                ref: CollectionItemRef(
                  type: CollectionItemRef.typeMarker,
                  uuid: m.uuid,
                ),
                collectionState: collectionState,
                visibility: visibility,
              ))
          .toList();
    }

    Marker buildMarker(marker_model.Marker location, double offsetY) {
      final namedIcon = iconService.getIcon(context, location.icon);
      const double markerScale = 1.0;
      const double markerHeight = MapMarkerPin.baseHeight * markerScale;
      final isSelected = selection.contains(location.uuid);
      return Marker(
        width: MapMarkerPin.baseWidth * markerScale,
        height: markerHeight,
        point: location.position,
        alignment: Alignment.bottomCenter,
        child: Transform.translate(
          offset: Offset(0, offsetY),
          child: MapMarkerPin(
            icon: location.icon != null ? namedIcon.icon : null,
            title: location.title,
            isSelected: isSelected,
            onTap: () => _handleMarkerTap(context, location),
            onLongPress: () => ref
                .read(markerSelectionProvider.notifier)
                .toggle(location.uuid),
            scale: markerScale,
          ),
        ),
      );
    }

    return viewportMarkersAsync.when(
      data: (locations) => MarkerLayer(
        markers: applyFilter(locations)
            .map((l) =>
                buildMarker(l, -MapMarkerPin.baseHeight))
            .toList(),
      ),
      loading: () {
        final previousData =
            ref.read(viewportMarkerNotifierProvider).asData?.value;
        if (previousData != null && previousData.isNotEmpty) {
          return MarkerLayer(
            markers: applyFilter(previousData)
                .map((l) => buildMarker(l, -MapMarkerPin.baseHeight / 2))
                .toList(),
          );
        }
        return const SizedBox.shrink();
      },
      error: (error, stack) {
        _log.warning('Error loading viewport markers', error, stack);
        return const SizedBox.shrink();
      },
    );
  }

  void _handleMarkerTap(BuildContext context, marker_model.Marker location) {
    final selection = ref.read(markerSelectionProvider);
    // While selection mode is active, a plain tap toggles selection rather
    // than opening the info sheet — matches the desktop-mail / files paradigm.
    if (selection.isNotEmpty) {
      ref.read(markerSelectionProvider.notifier).toggle(location.uuid);
      return;
    }
    _showInfoSheet(context, ref, location);
  }

  void _showInfoSheet(
      BuildContext context, WidgetRef ref, marker_model.Marker marker) async {
    final l10n = context.l10n;
    final result = await showExclusiveSheet<MarkerInfoResult>(
      context,
      builder: (_) => MarkerInfoSheet(marker: marker),
    );
    if (!context.mounted) return;
    if (result != null) {
      ref.read(viewportMarkerNotifierProvider.notifier).invalidateCache();
      _updateViewportMarkers();
    }
    if (result == MarkerInfoResult.updated) {
      AppSnackbars.success(context, l10n.markerUpdated);
    } else if (result == MarkerInfoResult.deleted) {
      AppSnackbars.success(context, l10n.markerDeleted);
    }
  }
}
