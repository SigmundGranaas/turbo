import 'package:latlong2/latlong.dart';
import 'package:turbo/features/measuring/models/distance_calculator.dart'
as calc;
import 'package:turbo/features/measuring/models/measure_point.dart';
import 'package:turbo/features/measuring/models/measure_point_type.dart';

class MeasurePointCollection {
  final List<MeasurePoint> _points;
  final calc.DistanceCalculator _calculator;
  double _totalDistance;

  MeasurePointCollection({
    List<MeasurePoint>? initialPoints,
    calc.DistanceCalculator? calculator,
  })  : _points = initialPoints ?? [],
        _calculator = calculator ?? calc.DistanceCalculator(),
        _totalDistance = 0;

  List<MeasurePoint> get points => List.unmodifiable(_points);
  double get totalDistance => _totalDistance;

  void addPoint(LatLng point) {
    if (_points.isEmpty) {
      _points.add(MeasurePoint(point: point, type: MeasurePointType.start));
      _updateTotalDistance();
      return;
    }

    // The current last point is no longer the end. Change its type to middle.
    // The start point's type should not be changed.
    final lastPoint = _points.last;
    if (lastPoint.type == MeasurePointType.end) {
      _points[_points.length - 1] = MeasurePoint(
        point: lastPoint.point,
        type: MeasurePointType.middle,
      );
    }

    // Add the new point, which is always the new end point.
    _points.add(MeasurePoint(point: point, type: MeasurePointType.end));
    _updateTotalDistance();
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

    // The new last point becomes the end point, unless it's the start point.
    if (_points.isNotEmpty && _points.last.type == MeasurePointType.middle) {
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