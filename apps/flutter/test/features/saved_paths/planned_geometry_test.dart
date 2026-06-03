import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/saved_paths/api.dart';

void main() {
  group('SavedPath plannedGeometry round-trip', () {
    final points = [const LatLng(60.0, 10.0), const LatLng(60.1, 10.1)];
    final planned = [
      const LatLng(60.0, 10.0),
      const LatLng(60.05, 10.05),
      const LatLng(60.1, 10.1),
    ];

    test('serialises and restores the planned geometry', () {
      final path = SavedPath(
        title: 'Hike',
        points: points,
        distance: 1234,
        plannedGeometry: planned,
      );
      final restored = SavedPath.fromLocalMap(path.toLocalMap());
      expect(restored.plannedGeometry, isNotNull);
      expect(restored.plannedGeometry!.length, planned.length);
      expect(restored.plannedGeometry!.first.latitude, closeTo(60.0, 1e-9));
      expect(restored.plannedGeometry!.last.longitude, closeTo(10.1, 1e-9));
    });

    test('null planned geometry round-trips as null', () {
      final path = SavedPath(title: 'Hike', points: points, distance: 1);
      final restored = SavedPath.fromLocalMap(path.toLocalMap());
      expect(restored.plannedGeometry, isNull);
    });

    test('copyWith can clear and set planned geometry', () {
      final path = SavedPath(
        title: 'Hike',
        points: points,
        distance: 1,
        plannedGeometry: planned,
      );
      expect(path.copyWith(clearPlannedGeometry: true).plannedGeometry, isNull);
      final reset =
          SavedPath(title: 'H', points: points, distance: 1).copyWith(
        plannedGeometry: planned,
      );
      expect(reset.plannedGeometry, planned);
    });
  });
}
