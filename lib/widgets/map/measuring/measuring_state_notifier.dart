import 'measure_point.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'measure_point_collection.dart';

class MeasuringState {
  final List<MeasurePoint> points;
  final double totalDistance;
  final bool isMapReady;

  const MeasuringState({
    required this.points,
    required this.totalDistance,
    required this.isMapReady,
  });

  MeasuringState copyWith({
    List<MeasurePoint>? points,
    double? totalDistance,
    bool? isMapReady,
  }) {
    return MeasuringState(
      points: points ?? this.points,
      totalDistance: totalDistance ?? this.totalDistance,
      isMapReady: isMapReady ?? this.isMapReady,
    );
  }
}

class MeasuringStateNotifier extends StateNotifier<MeasuringState> {
  final MeasurePointCollection _pointsManager;

  MeasuringStateNotifier({
    required LatLng startPoint,
    MeasurePointCollection? pointsManager,
  }) : _pointsManager = pointsManager ?? MeasurePointCollection(),
        super(const MeasuringState(
        points: [],
        totalDistance: 0,
        isMapReady: false,
      )) {
    _pointsManager.reset(startPoint);
    _updateState();
  }

  void setMapReady(bool ready) {
    state = state.copyWith(isMapReady: ready);
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

  void reset(LatLng startPoint) {
    _pointsManager.reset(startPoint);
    _updateState();
  }

  void _updateState() {
    state = state.copyWith(
      points: _pointsManager.points,
      totalDistance: _pointsManager.totalDistance,
    );
  }
}