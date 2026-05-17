import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/sharing/api.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/sharing/api.dart';

const _base = 'https://example.test';

ProviderContainer _container() {
  final c = ProviderContainer();
  addTearDown(c.dispose);
  return c;
}

void main() {
  group('ShareRouteHandler.handle', () {
    test('decodes a marker URL and pushes it to pendingShareProvider', () {
      final container = _container();
      final url = ShareableLinkCodec.encodeMarker(
        Marker(title: 'X', position: const LatLng(60, 10)),
        _base,
      );
      final handled = ShareRouteHandler(container)
          .handle(Uri.parse(url));

      expect(handled, isTrue);
      final pending = container.read(pendingShareProvider);
      expect(pending, isA<SharedMarkerPayload>());
      expect((pending as SharedMarkerPayload).marker.title, 'X');
    });

    test('decodes a path URL and pushes it to pendingShareProvider', () {
      final container = _container();
      final url = ShareableLinkCodec.encodePath(
        SavedPath(
          title: 'P',
          points: const [LatLng(60, 10), LatLng(60.1, 10.1)],
          distance: 1,
        ),
        _base,
      );
      final handled = ShareRouteHandler(container)
          .handle(Uri.parse(url));

      expect(handled, isTrue);
      expect(container.read(pendingShareProvider), isA<SharedPathPayload>());
    });

    test('returns false and leaves state untouched for unrelated URLs', () {
      final container = _container();
      final handled = ShareRouteHandler(container)
          .handle(Uri.parse('https://example.test/login'));

      expect(handled, isFalse);
      expect(container.read(pendingShareProvider), isNull);
    });

    test('swallows invalid share URLs (logs but does not throw)', () {
      final container = _container();
      final handled = ShareRouteHandler(container)
          .handle(Uri.parse('https://example.test/share/m?d=corrupt!'));

      expect(handled, isFalse);
      expect(container.read(pendingShareProvider), isNull);
    });
  });
}
