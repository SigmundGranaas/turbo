import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/measuring/data/measure_point_collection.dart';
import 'package:turbo/features/measuring/data/measuring_state.dart';
import 'package:turbo/features/measuring/models/measure_point.dart';
import 'package:turbo/features/measuring/models/measure_point_type.dart';

class MeasuringStateNotifier extends Notifier<MeasuringState> {
  late MeasurePointCollection _pointsManager;
  late LatLng _initialStartPoint;

  // Internal setter for family initialization
  set initialStartPoint(LatLng value) => _initialStartPoint = value;

  @override
  MeasuringState build() {
    // This will be called by Riverpod after the instance is created
    // and initialStartPoint has been set by the provider.
    _pointsManager = MeasurePointCollection();
    _pointsManager.reset(_initialStartPoint);
    
    return MeasuringState(
      points: [MeasurePoint(point: _initialStartPoint, type: MeasurePointType.start)],
      totalDistance: 0,
      isDrawing: false,
      isSmoothing: false,
      showIntermediatePoints: true,
    );
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

  void _updateState() {
    state = state.copyWith(
      points: _pointsManager.points,
      totalDistance: _pointsManager.totalDistance,
    );
  }
}