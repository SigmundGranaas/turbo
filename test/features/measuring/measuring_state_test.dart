import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart' hide DistanceCalculator;
import 'package:turbo/features/measuring/data/measure_point_collection.dart';
import 'package:turbo/features/measuring/models/distance_calculator.dart';
import 'package:turbo/features/measuring/models/measure_point_type.dart';
import 'package:turbo/features/measuring/widgets/measuring_map_page.dart';

/// A fake implementation of [DistanceCalculator] for testing purposes.
/// It returns a predictable, constant value for distance calculations.
class FakeDistanceCalculator implements DistanceCalculator {
  final double distanceToReturn;

  FakeDistanceCalculator({this.distanceToReturn = 1000.0});

  @override
  double calculateDistance(LatLng point1, LatLng point2) {
    return distanceToReturn;
  }

  @override
  double calculateTotalDistance(List<LatLng> points) {
    if (points.length < 2) return 0;
    return (points.length - 1) * distanceToReturn;
  }
}

void main() {
  group('MeasurePointCollection', () {
    const point1 = LatLng(0, 0);
    const point2 = LatLng(1, 1);
    const point3 = LatLng(2, 2);

    late FakeDistanceCalculator fakeCalculator;

    setUp(() {
      fakeCalculator = FakeDistanceCalculator(distanceToReturn: 1000.0);
    });

    test('initial state is empty', () {
      final collection = MeasurePointCollection();
      expect(collection.points, isEmpty);
      expect(collection.totalDistance, 0.0);
    });

    test('addPoint correctly sets point types and calculates distance', () {
      final collection = MeasurePointCollection(calculator: fakeCalculator);

      collection.addPoint(point1);
      expect(collection.points.length, 1);
      expect(collection.points.first.type, MeasurePointType.start);
      expect(collection.totalDistance, 0.0);

      collection.addPoint(point2);
      expect(collection.points.length, 2);
      expect(collection.points[0].type, MeasurePointType.start);
      expect(collection.points[1].type, MeasurePointType.end);
      expect(collection.totalDistance, 1000.0);

      collection.addPoint(point3);
      expect(collection.points.length, 3);
      expect(collection.points[0].type, MeasurePointType.start);
      expect(collection.points[1].type, MeasurePointType.middle);
      expect(collection.points[2].type, MeasurePointType.end);
      expect(collection.totalDistance, 2000.0);
    });

    test('undoLastPoint correctly removes point and updates distance', () {
      final collection = MeasurePointCollection(calculator: fakeCalculator);
      collection.addPoint(point1);
      collection.addPoint(point2);
      collection.addPoint(point3);
      expect(collection.totalDistance, 2000.0);

      final success = collection.undoLastPoint();

      expect(success, isTrue);
      expect(collection.points.length, 2);
      expect(collection.points.last.type, MeasurePointType.end);
      expect(collection.totalDistance, 1000.0);
    });

    test('undoLastPoint with one point succeeds and results in 0 points', () {
      final collection = MeasurePointCollection(calculator: fakeCalculator);
      collection.addPoint(point1);

      final success = collection.undoLastPoint();

      expect(success, isTrue);
      expect(collection.points, isEmpty);
      expect(collection.totalDistance, 0.0);
    });

    test('undoLastPoint on empty collection returns false', () {
      final collection = MeasurePointCollection(calculator: fakeCalculator);

      final success = collection.undoLastPoint();

      expect(success, isFalse);
      expect(collection.points, isEmpty);
      expect(collection.totalDistance, 0.0);
    });

    test('clear empties collection and resets distance', () {
      final collection = MeasurePointCollection(calculator: fakeCalculator);
      collection.addPoint(point1);
      collection.addPoint(point2);
      collection.addPoint(point3);
      expect(collection.points.length, 3);
      expect(collection.totalDistance, 2000.0);

      collection.clear();

      expect(collection.points, isEmpty);
      expect(collection.totalDistance, 0.0);
    });

    test('reset clears points and sets a new start point', () {
      final collection = MeasurePointCollection(calculator: fakeCalculator);
      collection.addPoint(point1);
      collection.addPoint(point2);

      collection.reset(point3);

      expect(collection.points.length, 1);
      expect(collection.points.first.point, point3);
      expect(collection.points.first.type, MeasurePointType.start);
      expect(collection.totalDistance, 0.0);
    });
  });

  group('MeasuringStateNotifier', () {
    test('initial state has empty points and default flags', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final notifier =
      container.read(measuringStateProvider.notifier);
      final state = notifier.state;

      expect(state.points, isEmpty);
      expect(state.totalDistance, 0);
      expect(state.isDrawing, false);
      expect(state.isSmoothing, false);
      expect(state.showIntermediatePoints, true);
    });

    test('toggle methods correctly update boolean flags', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      final notifier =
      container.read(measuringStateProvider.notifier);

      // Test Drawing
      expect(notifier.state.isDrawing, isFalse);
      notifier.toggleDrawing();
      expect(notifier.state.isDrawing, isTrue);
      notifier.toggleDrawing();
      expect(notifier.state.isDrawing, isFalse);

      // Test Smoothing
      expect(notifier.state.isSmoothing, isFalse);
      notifier.toggleSmoothing();
      expect(notifier.state.isSmoothing, isTrue);

      // Test Intermediate Points
      expect(notifier.state.showIntermediatePoints, isTrue);
      notifier.toggleIntermediatePoints();
      expect(notifier.state.showIntermediatePoints, isFalse);
    });
  });
}
