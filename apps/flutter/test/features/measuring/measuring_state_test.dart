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
    ProviderContainer makeContainer() {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      // Pin the autoDispose provider for the duration of the test.
      container.listen(measuringStateProvider, (_, _) {});
      return container;
    }

    test('initial state has empty points and default flags', () {
      final container = makeContainer();
      final state = container.read(measuringStateProvider);

      expect(state.points, isEmpty);
      expect(state.totalDistance, 0);
      expect(state.isDrawing, false);
    });

    test('toggle drawing correctly updates flag', () {
      final container = makeContainer();
      final notifier = container.read(measuringStateProvider.notifier);

      expect(container.read(measuringStateProvider).isDrawing, isFalse);
      notifier.toggleDrawing();
      expect(container.read(measuringStateProvider).isDrawing, isTrue);
      notifier.toggleDrawing();
      expect(container.read(measuringStateProvider).isDrawing, isFalse);
    });

    test('addPoint accumulates points and recalculates totalDistance', () {
      final container = makeContainer();
      final notifier = container.read(measuringStateProvider.notifier);

      notifier.addPoint(const LatLng(59.9, 10.7));
      expect(container.read(measuringStateProvider).points.length, 1);
      expect(container.read(measuringStateProvider).totalDistance, 0);

      notifier.addPoint(const LatLng(60.0, 10.7));
      final twoPointState = container.read(measuringStateProvider);
      expect(twoPointState.points.length, 2);
      // Distance is calculated by the real DistanceCalculator (~11 km).
      expect(twoPointState.totalDistance, greaterThan(10000));
      expect(twoPointState.totalDistance, lessThan(12000));

      notifier.addPoint(const LatLng(60.1, 10.7));
      expect(container.read(measuringStateProvider).points.length, 3);
      expect(
          container.read(measuringStateProvider).totalDistance,
          greaterThan(twoPointState.totalDistance));
    });

    test('undoLastPoint removes the most recent point and updates distance',
        () {
      final container = makeContainer();
      final notifier = container.read(measuringStateProvider.notifier);

      notifier.addPoint(const LatLng(59.9, 10.7));
      notifier.addPoint(const LatLng(60.0, 10.7));
      notifier.addPoint(const LatLng(60.1, 10.7));
      final threeDistance =
          container.read(measuringStateProvider).totalDistance;

      notifier.undoLastPoint();
      final twoState = container.read(measuringStateProvider);
      expect(twoState.points.length, 2);
      expect(twoState.totalDistance, lessThan(threeDistance));
    });

    test('undoLastPoint on empty state is a safe no-op', () {
      final container = makeContainer();
      final notifier = container.read(measuringStateProvider.notifier);

      // Should not throw.
      notifier.undoLastPoint();
      expect(container.read(measuringStateProvider).points, isEmpty);
      expect(container.read(measuringStateProvider).totalDistance, 0);
    });

    test('reset empties points and zeroes distance', () {
      final container = makeContainer();
      final notifier = container.read(measuringStateProvider.notifier);

      notifier.addPoint(const LatLng(59.9, 10.7));
      notifier.addPoint(const LatLng(60.0, 10.7));
      expect(container.read(measuringStateProvider).points, hasLength(2));

      notifier.reset();
      final state = container.read(measuringStateProvider);
      expect(state.points, isEmpty);
      expect(state.totalDistance, 0);
    });

    test('reset returns to the initial state — clears points AND exits '
        'drawing mode', () {
      final container = makeContainer();
      final notifier = container.read(measuringStateProvider.notifier);
      notifier.toggleDrawing();
      notifier.addPoint(const LatLng(59.9, 10.7));
      expect(container.read(measuringStateProvider).isDrawing, isTrue);

      notifier.reset();
      final state = container.read(measuringStateProvider);
      expect(state.isDrawing, isFalse);
      expect(state.points, isEmpty);
      expect(state.totalDistance, 0);
    });
  });
}
