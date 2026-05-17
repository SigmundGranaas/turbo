import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/sharing/api.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/sharing/api.dart';

void main() {
  group('pendingShareProvider', () {
    test('starts as null', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      expect(container.read(pendingShareProvider), isNull);
    });

    test('push() stores the payload', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final payload = SharedMarkerPayload(Marker(
        title: 'X',
        position: const LatLng(60, 10),
      ));
      container.read(pendingShareProvider.notifier).push(payload);

      expect(container.read(pendingShareProvider), same(payload));
    });

    test('consume() returns and clears the payload', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final payload = SharedMarkerPayload(Marker(
        title: 'X',
        position: const LatLng(60, 10),
      ));
      final notifier = container.read(pendingShareProvider.notifier);
      notifier.push(payload);

      expect(notifier.consume(), same(payload));
      expect(container.read(pendingShareProvider), isNull);
      expect(notifier.consume(), isNull);
    });
  });
}
