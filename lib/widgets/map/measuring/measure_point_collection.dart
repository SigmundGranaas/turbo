
import 'package:latlong2/latlong.dart';

import 'distance_calculator.dart' as calc;
import 'measure_point.dart';
import 'measure_point_type.dart';

class MeasurePointCollection {
  final List<MeasurePoint> _points;
  final calc.DistanceCalculator _calculator;
  double _totalDistance;

  MeasurePointCollection({
    List<MeasurePoint>? initialPoints,
    calc.DistanceCalculator? calculator,
  }) : _points = initialPoints ?? [],
        _calculator = calculator ?? calc.DistanceCalculator(),
        _totalDistance = 0;

  List<MeasurePoint> get points => List.unmodifiable(_points);
  double get totalDistance => _totalDistance;

  void addPoint(LatLng point) {
    final newType = _determinePointType();
    _updateLastPointTypeIfNeeded(newType);

    _points.add(MeasurePoint(point: point, type: newType));
    _updateTotalDistance();
  }

  MeasurePointType _determinePointType() {
    if (_points.isEmpty) return MeasurePointType.start;
    if (_points.length == 1) return MeasurePointType.end;
    return MeasurePointType.middle;
  }

  void _updateLastPointTypeIfNeeded(MeasurePointType newType) {
    if (newType == MeasurePointType.middle && _points.isNotEmpty) {
      final lastIndex = _points.length - 1;
      _points[lastIndex] = MeasurePoint(
        point: _points[lastIndex].point,
        type: MeasurePointType.middle,
      );
    }
  }

  void _updateTotalDistance() {
    if (_points.length < 2) {
      _totalDistance = 0;
      return;
    }

    _totalDistance += _calculator.calculateDistance(
      _points[_points.length - 2].point,
      _points.last.point,
    );
  }

  bool undoLastPoint() {
    if (_points.length <= 1) return false;

    final removedPoint = _points.removeLast();

    if (_points.length > 1) {
      final lastIndex = _points.length - 1;
      _points[lastIndex] = MeasurePoint(
        point: _points[lastIndex].point,
        type: MeasurePointType.end,
      );
    }

    _totalDistance -= _calculator.calculateDistance(
      _points.last.point,
      removedPoint.point,
    );

    return true;
  }

  void reset(LatLng startPoint) {
    _points.clear();
    _points.add(MeasurePoint(point: startPoint, type: MeasurePointType.start));
    _totalDistance = 0;
  }
}