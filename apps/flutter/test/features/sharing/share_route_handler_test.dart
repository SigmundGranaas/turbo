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

    test('recognises /share/r/<token> and pushes onto the link-redemption provider', () {
      final container = _container();
      final handled = ShareRouteHandler(container)
          .handle(Uri.parse('https://example.test/share/r/abc123'));

      expect(handled, isTrue);
      expect(container.read(pendingLinkRedemptionProvider), 'abc123');
      // Stateless payload provider must NOT be populated for tracked links.
      expect(container.read(pendingShareProvider), isNull);
    });

    test('/share/r without a token segment is not recognised', () {
      final container = _container();
      final handled = ShareRouteHandler(container)
          .handle(Uri.parse('https://example.test/share/r/'));

      expect(handled, isFalse);
      expect(container.read(pendingLinkRedemptionProvider), isNull);
    });
  });

  group('PendingLinkRedemptionNotifier', () {
    test('take() returns the token and clears state', () {
      final container = _container();
      final notifier =
          container.read(pendingLinkRedemptionProvider.notifier);

      notifier.push('tok1');
      expect(notifier.take(), 'tok1');
      expect(container.read(pendingLinkRedemptionProvider), isNull);
      expect(notifier.take(), isNull);
    });
  });
}
