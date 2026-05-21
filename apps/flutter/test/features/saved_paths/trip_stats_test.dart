import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/saved_paths/api.dart';

SavedPath _path({
  required String uuid,
  required double distance,
  double? ascent,
  int? movingSeconds,
  DateTime? recordedAt,
}) =>
    SavedPath(
      uuid: uuid,
      title: 't',
      points: const [LatLng(0, 0), LatLng(0.01, 0.01)],
      distance: distance,
      ascent: ascent,
      movingTimeSeconds: movingSeconds,
      recordedAt: recordedAt,
    );

void main() {
  group('TripStats.from', () {
    test('empty iterable yields zeros', () {
      expect(TripStats.from(const []).totalPaths, 0);
      expect(TripStats.from(const []).totalDistanceMeters, 0);
    });

    test('sums distance, ascent, moving time across paths', () {
      final stats = TripStats.from([
        _path(uuid: 'a', distance: 1000, ascent: 50, movingSeconds: 600),
        _path(uuid: 'b', distance: 2500, ascent: 120, movingSeconds: 1800),
      ]);
      expect(stats.totalPaths, 2);
      expect(stats.totalDistanceMeters, 3500);
      expect(stats.totalAscentMeters, 170);
      expect(stats.totalMovingTimeSeconds, 2400);
      expect(stats.longestPathMeters, 2500);
    });

    test('paths without recording fields do not break aggregation', () {
      final stats = TripStats.from([
        _path(uuid: 'a', distance: 1000),
        _path(uuid: 'b', distance: 500, ascent: 25, movingSeconds: 300),
      ]);
      expect(stats.totalPaths, 2);
      expect(stats.recordedPaths, 0);
      expect(stats.totalAscentMeters, 25);
      expect(stats.totalMovingTimeSeconds, 300);
    });

    test('distinct recording days deduplicates same-day recordings', () {
      final stats = TripStats.from([
        _path(uuid: 'a', distance: 1, recordedAt: DateTime.utc(2026, 5, 17)),
        _path(uuid: 'b', distance: 1, recordedAt: DateTime.utc(2026, 5, 17)),
        _path(uuid: 'c', distance: 1, recordedAt: DateTime.utc(2026, 5, 18)),
      ]);
      expect(stats.recordedPaths, 3);
      expect(stats.distinctRecordingDays, 2);
    });
  });
}
