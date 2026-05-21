import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/saved_paths/api.dart';

class _StubService implements HoydedataService {
  final List<double?> response;
  Object? error;
  int calls = 0;
  _StubService(this.response);

  @override
  Future<List<double?>> elevationsFor(List<LatLng> points) async {
    calls++;
    if (error != null) throw error!;
    return response.sublist(0, points.length);
  }
}

SavedPath _path({
  required int size,
  List<double>? elevations,
}) =>
    SavedPath(
      title: 'Trip',
      points: [
        for (var i = 0; i < size; i++) LatLng(60 + i * 0.001, 5 + i * 0.001),
      ],
      distance: size * 10.0,
      elevations: elevations,
    );

void main() {
  group('backfillElevations', () {
    test('returns notNeeded when the path already has full elevations',
        () async {
      final svc = _StubService(const []);
      final result = await backfillElevations(
        _path(size: 5, elevations: const [10, 11, 12, 13, 14]),
        svc,
      );
      expect(result.status, ElevationBackfillStatus.notNeeded);
      expect(svc.calls, 0);
    });

    test('returns notNeeded when the path has no points', () async {
      final svc = _StubService(const []);
      final emptyPath = SavedPath(
        title: 't',
        points: const [],
        distance: 0,
      );
      final result = await backfillElevations(emptyPath, svc);
      expect(result.status, ElevationBackfillStatus.notNeeded);
      expect(svc.calls, 0);
    });

    test('fills in missing elevations and recomputes ascent / descent',
        () async {
      final svc =
          _StubService(const [100.0, 105.0, 110.0, 115.0, 120.0]);
      final result = await backfillElevations(
        _path(size: 5),
        svc,
      );
      expect(result.status, ElevationBackfillStatus.filled);
      expect(result.path.elevations, isNotNull);
      expect(result.path.elevations!.length, 5);
      expect(result.path.ascent, greaterThan(0));
    });

    test('failed fetch returns failed status and leaves path unchanged',
        () async {
      final svc = _StubService(const [])..error = const HoydedataServiceException(500, 'boom');
      final original = _path(size: 5);
      final result = await backfillElevations(original, svc);
      expect(result.status, ElevationBackfillStatus.failed);
      expect(result.path, same(original));
    });

    test('partial fill: some values null comes back as partial', () async {
      final svc =
          _StubService([100.0, null, 110.0, null, 120.0]);
      final result = await backfillElevations(_path(size: 5), svc);
      expect(result.status, ElevationBackfillStatus.partial);
    });
  });
}
