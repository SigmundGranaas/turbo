import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/routing/api.dart';

void main() {
  late ProviderContainer container;
  setUp(() => container = ProviderContainer());
  tearDown(() => container.dispose());

  RoutePlanningNotifier notifier() =>
      container.read(routePlanningProvider.notifier);
  List<LatLng> wps() => container.read(routePlanningProvider).waypoints;

  group('RoutePlanningNotifier.insertWaypoint', () {
    test('appends when there are fewer than two stops', () {
      notifier().insertWaypoint(const LatLng(60.0, 10.0));
      expect(wps(), [const LatLng(60.0, 10.0)]);
    });

    test('inserts a via-point between the two surrounding stops', () {
      notifier()
        ..addWaypoint(const LatLng(60.0, 10.0))
        ..addWaypoint(const LatLng(60.0, 10.2));
      // A point near the A–B midpoint adds least detour between them.
      notifier().insertWaypoint(const LatLng(60.0, 10.1));
      expect(wps().length, 3);
      expect(wps()[1], const LatLng(60.0, 10.1));
    });

    test('chooses the lowest-detour gap among several stops', () {
      notifier()
        ..addWaypoint(const LatLng(60.0, 10.0)) // A (0)
        ..addWaypoint(const LatLng(60.0, 10.1)) // B (1)
        ..addWaypoint(const LatLng(60.0, 10.2)); // C (2)
      // Near the B–C segment → should land at index 2 (between B and C).
      notifier().insertWaypoint(const LatLng(60.0, 10.15));
      expect(wps().length, 4);
      expect(wps()[2], const LatLng(60.0, 10.15));
      expect(wps().first, const LatLng(60.0, 10.0));
      expect(wps().last, const LatLng(60.0, 10.2));
    });
  });

  group('RoutePlanningNotifier edit basics', () {
    test('moveWaypoint repositions a stop in place', () {
      notifier()
        ..addWaypoint(const LatLng(60.0, 10.0))
        ..addWaypoint(const LatLng(60.0, 10.2));
      notifier().moveWaypoint(1, const LatLng(61.0, 11.0));
      expect(wps()[1], const LatLng(61.0, 11.0));
      expect(wps().length, 2);
    });

    test('removeAt drops the indexed stop', () {
      notifier()
        ..addWaypoint(const LatLng(60.0, 10.0))
        ..addWaypoint(const LatLng(60.0, 10.1))
        ..addWaypoint(const LatLng(60.0, 10.2));
      notifier().removeAt(1);
      expect(wps(), [const LatLng(60.0, 10.0), const LatLng(60.0, 10.2)]);
    });
  });
}
