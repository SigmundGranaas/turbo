import 'package:flutter/gestures.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/measuring/data/measure_point_collection.dart';
import 'package:turbo/features/measuring/data/measuring_state.dart';
import 'package:turbo/features/settings/api.dart';

import 'freehand_gesture_handler.dart';

class MeasuringStateNotifier extends Notifier<MeasuringState> {
  late MeasurePointCollection _pointsManager;

  /// Freehand drawing state lives here (not in a transient widget) so it
  /// survives across pointer events while the measuring tool is mounted on the
  /// shared map. Edge-pan-during-draw (a desktop-only nicety) is intentionally
  /// dropped in the in-place tool.
  late final FreehandGestureHandler _freehand;

  @override
  MeasuringState build() {
    _pointsManager = MeasurePointCollection();
    _freehand = FreehandGestureHandler(
      notifier: this,
      getSensitivity: () =>
          ref.read(settingsProvider).value?.drawSensitivity ?? 15.0,
      onEdgePanStop: () {},
      onMove: (_) {},
    );
    return MeasuringState.initial();
  }

  void handlePointerDown(PointerDownEvent e, LatLng p) =>
      _freehand.handlePointerDown(e, p);
  void handlePointerMove(PointerMoveEvent e, LatLng p) =>
      _freehand.handlePointerMove(e, p);
  void handlePointerUp(PointerUpEvent e, LatLng p) =>
      _freehand.handlePointerUp(e, p);

  void addPoint(LatLng point) {
    _pointsManager.addPoint(point);
    _updateState();
  }

  void undoLastPoint() {
    if (_pointsManager.undoLastPoint()) {
      _updateState();
    }
  }

  void reset() {
    _pointsManager.clear();
    _freehand.reset();
    state = MeasuringState.initial();
  }

  void toggleDrawing() {
    state = state.copyWith(isDrawing: !state.isDrawing);
  }

  void _updateState() {
    state = state.copyWith(
      points: _pointsManager.points,
      totalDistance: _pointsManager.totalDistance,
    );
  }
}
