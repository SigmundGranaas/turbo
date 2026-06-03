import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/search/data/composite_search_service.dart';
import 'package:turbo/features/search/data/location_service.dart';

import '../../helpers/fakes/fake_location_service.dart';

LocationSearchResult _r(String title, String source) => LocationSearchResult(
      title: title,
      position: const LatLng(0, 0),
      source: source,
    );

void main() {
  group('CompositeSearchService', () {
    test('concatenates results from all three sub-services in marker → path → '
        'kartverket order', () async {
      final markers = FakeLocationService(results: [_r('Marker 1', 'local')]);
      final paths = FakeLocationService(results: [_r('Path 1', 'path')]);
      final kartverket =
          FakeLocationService(results: [_r('Place 1', 'kartverket')]);

      final composite = CompositeSearchService(
          kartverket, markers, paths, FakeLocationService(), FakeLocationService());
      final results = await composite.findLocationsBy('foo');

      expect(results.map((r) => r.source).toList(),
          ['local', 'path', 'kartverket']);
      expect(results.map((r) => r.title).toList(),
          ['Marker 1', 'Path 1', 'Place 1']);
    });

    test('passes the query to every sub-service', () async {
      final markers = FakeLocationService();
      final paths = FakeLocationService();
      final kartverket = FakeLocationService();

      final composite = CompositeSearchService(
          kartverket, markers, paths, FakeLocationService(), FakeLocationService());
      await composite.findLocationsBy('bergen');

      expect(markers.queries, ['bergen']);
      expect(paths.queries, ['bergen']);
      expect(kartverket.queries, ['bergen']);
    });

    test('returns empty list when no service finds anything', () async {
      final composite = CompositeSearchService(
        FakeLocationService(),
        FakeLocationService(),
        FakeLocationService(),
        FakeLocationService(),
        FakeLocationService(),
      );
      final results = await composite.findLocationsBy('xyz');
      expect(results, isEmpty);
    });

    test('an error in one sub-service surfaces — Future.wait fails fast',
        () async {
      // Current implementation does not catch per-service errors; document
      // that contract here so a future change is conscious.
      final markers = FakeLocationService(throwOnQuery: StateError('local boom'));
      final paths = FakeLocationService(results: [_r('Path', 'path')]);
      final kartverket = FakeLocationService(results: [_r('Place', 'kartverket')]);

      final composite = CompositeSearchService(
          kartverket, markers, paths, FakeLocationService(), FakeLocationService());
      expect(() => composite.findLocationsBy('foo'),
          throwsA(isA<StateError>()));
    });

    test('runs sub-services concurrently — total time ≈ max of individual times',
        () async {
      Future<List<LocationSearchResult>> slow(Duration d, String label) async {
        await Future.delayed(d);
        return [_r(label, label)];
      }

      final markers = FakeLocationService(
        responder: (_) => slow(const Duration(milliseconds: 100), 'local'),
      );
      final paths = FakeLocationService(
        responder: (_) => slow(const Duration(milliseconds: 100), 'path'),
      );
      final kartverket = FakeLocationService(
        responder: (_) => slow(const Duration(milliseconds: 100), 'kartverket'),
      );

      final composite = CompositeSearchService(
          kartverket, markers, paths, FakeLocationService(), FakeLocationService());
      final sw = Stopwatch()..start();
      await composite.findLocationsBy('foo');
      sw.stop();
      // Three serial 100ms calls would be 300ms; concurrent should be well
      // below 250ms. Generous bound to keep the test stable on CI.
      expect(sw.elapsedMilliseconds, lessThan(250));
    });
  });
}
