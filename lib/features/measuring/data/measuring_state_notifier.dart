import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/measuring/data/measure_point_collection.dart';
import 'package:turbo/features/measuring/data/measuring_state.dart';
import 'package:turbo/features/measuring/models/measure_point.dart';
import 'package:turbo/features/measuring/models/measure_point_type.dart';

class MeasuringStateNotifier extends StateNotifier<MeasuringState> {
  final MeasurePointCollection _pointsManager;
  final LatLng _initialStartPoint;

  MeasuringStateNotifier({
    required LatLng startPoint,
    MeasurePointCollection? pointsManager,
  })  : _pointsManager = pointsManager ?? MeasurePointCollection(),
        _initialStartPoint = startPoint,
  // The state is now initialized atomically with its correct first value.
  // It is never in a transient "empty" state. This resolves the test race condition.
        super(
        MeasuringState(
          points: [
            MeasurePoint(point: startPoint, type: MeasurePointType.start)
          ],
          totalDistance: 0,
          isDrawing: false,
          isSmoothing: false,
          showIntermediatePoints: true,
          drawSensitivity: 15.0,
        ),
      ) {
    // Also ensure the internal manager is synced with this initial state.
    _pointsManager.reset(startPoint);
  }

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
    _pointsManager.reset(_initialStartPoint);
    _updateState();
  }

  void toggleSmoothing() {
    state = state.copyWith(isSmoothing: !state.isSmoothing);
  }

  void toggleDrawing() {
    state = state.copyWith(isDrawing: !state.isDrawing);
  }

  void toggleIntermediatePoints() {
    state = state.copyWith(showIntermediatePoints: !state.showIntermediatePoints);
  }

  void setDrawSensitivity(double sensitivity) {
    state = state.copyWith(drawSensitivity: sensitivity);
  }

  void _updateState() {
    state = state.copyWith(
      points: _pointsManager.points,
      totalDistance: _pointsManager.totalDistance,
    );
  }
}