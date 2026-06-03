import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

/// How the offline-download area is being chosen.
enum SelectionMode { viewport, rectangle, draw }

/// Immutable selection state for the in-place region-select tool. Mirrors the
/// fields the old `RegionCreationPage` held in widget state, lifted into a
/// provider so the tool's layers, overlay, pointer stream and rectangle
/// handles all share one source of truth.
class RegionSelectionState {
  final SelectionMode mode;
  final LatLngBounds? bounds;
  final List<LatLng> drawnPoints;
  final bool isDrawing;

  const RegionSelectionState({
    this.mode = SelectionMode.viewport,
    this.bounds,
    this.drawnPoints = const [],
    this.isDrawing = false,
  });

  bool get isValid =>
      bounds != null && (drawnPoints.length > 2 || mode != SelectionMode.draw);

  RegionSelectionState copyWith({
    SelectionMode? mode,
    LatLngBounds? bounds,
    bool clearBounds = false,
    List<LatLng>? drawnPoints,
    bool? isDrawing,
  }) {
    return RegionSelectionState(
      mode: mode ?? this.mode,
      bounds: clearBounds ? null : (bounds ?? this.bounds),
      drawnPoints: drawnPoints ?? this.drawnPoints,
      isDrawing: isDrawing ?? this.isDrawing,
    );
  }
}

class RegionSelectionNotifier extends Notifier<RegionSelectionState> {
  // Transient drag state — which rectangle corner (0=SW,1=NW,2=NE,3=SE) is
  // being dragged. Not in the immutable state since it only matters mid-drag.
  int? _draggedHandle;

  @override
  RegionSelectionState build() => const RegionSelectionState();

  /// Switch selection mode. [viewportBounds] is the map's current visible
  /// bounds, used to seed viewport / rectangle selections.
  void setMode(SelectionMode mode, LatLngBounds viewportBounds) {
    switch (mode) {
      case SelectionMode.viewport:
        state = state.copyWith(
            mode: mode, bounds: viewportBounds, drawnPoints: const []);
      case SelectionMode.rectangle:
        final latInset = (viewportBounds.north - viewportBounds.south) * 0.15;
        final lngInset = (viewportBounds.east - viewportBounds.west) * 0.15;
        state = state.copyWith(
          mode: mode,
          drawnPoints: const [],
          bounds: LatLngBounds(
            LatLng(viewportBounds.south + latInset,
                viewportBounds.west + lngInset),
            LatLng(viewportBounds.north - latInset,
                viewportBounds.east - lngInset),
          ),
        );
      case SelectionMode.draw:
        state = state.copyWith(
            mode: mode, drawnPoints: const [], clearBounds: true);
    }
  }

  /// Track the camera while in viewport mode.
  void updateViewport(LatLngBounds bounds) {
    if (state.mode == SelectionMode.viewport) {
      state = state.copyWith(bounds: bounds);
    }
  }

  void pointerDown(LatLng latlng) {
    state = state.copyWith(
        isDrawing: true, drawnPoints: [latlng], clearBounds: true);
  }

  void pointerMove(LatLng latlng) {
    if (!state.isDrawing) return;
    state = state.copyWith(drawnPoints: [...state.drawnPoints, latlng]);
  }

  void pointerUp() {
    if (!state.isDrawing) return;
    if (state.drawnPoints.length > 2) {
      state = state.copyWith(
          isDrawing: false,
          bounds: LatLngBounds.fromPoints(state.drawnPoints));
    } else {
      state = state.copyWith(
          isDrawing: false, drawnPoints: const [], clearBounds: true);
    }
  }

  void clearDrawing() {
    state = state.copyWith(drawnPoints: const [], clearBounds: true);
  }

  void startHandleDrag(int index) => _draggedHandle = index;
  void endHandleDrag() => _draggedHandle = null;

  /// Move the dragged rectangle corner to [point].
  void updateHandle(LatLng point) {
    final b = state.bounds;
    if (_draggedHandle == null || b == null) return;
    var west = b.west, east = b.east, north = b.north, south = b.south;
    switch (_draggedHandle) {
      case 0:
        south = point.latitude;
        west = point.longitude;
      case 1:
        north = point.latitude;
        west = point.longitude;
      case 2:
        north = point.latitude;
        east = point.longitude;
      case 3:
        south = point.latitude;
        east = point.longitude;
    }
    state = state.copyWith(
        bounds: LatLngBounds(LatLng(south, west), LatLng(north, east)));
  }

  void reset() {
    _draggedHandle = null;
    state = const RegionSelectionState();
  }
}

final regionSelectionProvider =
    NotifierProvider.autoDispose<RegionSelectionNotifier, RegionSelectionState>(
  RegionSelectionNotifier.new,
);
