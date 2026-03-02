import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/measuring/data/measure_point_collection.dart';
import 'package:turbo/features/measuring/data/measuring_state.dart';

class MeasuringStateNotifier extends Notifier<MeasuringState> {
  late MeasurePointCollection _pointsManager;

  @override
  MeasuringState build() {
    _pointsManager = MeasurePointCollection();
    return MeasuringState.initial();
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
    _pointsManager.clear();
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

  void _updateState() {
    state = state.copyWith(
      points: _pointsManager.points,
      totalDistance: _pointsManager.totalDistance,
    );
  }
}
