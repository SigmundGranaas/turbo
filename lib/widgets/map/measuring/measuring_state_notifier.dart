import 'measure_point.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'measure_point_collection.dart';

class MeasuringState {
  final List<MeasurePoint> points;
  final double totalDistance;

  const MeasuringState({
    required this.points,
    required this.totalDistance,
  });

  MeasuringState copyWith({
    List<MeasurePoint>? points,
    double? totalDistance,
  }) {
    return MeasuringState(
      points: points ?? this.points,
      totalDistance: totalDistance ?? this.totalDistance,
    );
  }
}

class MeasuringStateNotifier extends StateNotifier<MeasuringState> {
  final MeasurePointCollection _pointsManager;
  final LatLng _initialStartPoint;

  MeasuringStateNotifier({
    required LatLng startPoint,
    MeasurePointCollection? pointsManager,
  })  : _pointsManager = pointsManager ?? MeasurePointCollection(),
        _initialStartPoint = startPoint,
        super(const MeasuringState(
        points: [],
        totalDistance: 0,
      )) {
    reset(); // Initialize with the start point
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

  void _updateState() {
    state = state.copyWith(
      points: _pointsManager.points,
      totalDistance: _pointsManager.totalDistance,
    );
  }
}