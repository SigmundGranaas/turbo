import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/features/navigation/data/navigation_state_notifier.dart';

void main() {
  group('NavigationStateNotifier', () {
    test('initial state is inactive with no target', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final state = container.read(navigationStateProvider);
      expect(state.isActive, isFalse);
      expect(state.target, isNull);
    });

    test('startNavigation activates state with the given target', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      const target = LatLng(59.9, 10.7);
      container.read(navigationStateProvider.notifier).startNavigation(target);

      final state = container.read(navigationStateProvider);
      expect(state.isActive, isTrue);
      expect(state.target, target);
    });

    test('startNavigation enables followMode as a side-effect — cross-feature '
        'wiring is exercised', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      // Read followMode first so the notifier exists and we can observe it.
      expect(container.read(followModeProvider), FollowMode.off);

      container
          .read(navigationStateProvider.notifier)
          .startNavigation(const LatLng(0, 0));

      expect(container.read(followModeProvider), FollowMode.active);
    });

    test('stopNavigation returns to NavigationState.inactive', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      final notifier = container.read(navigationStateProvider.notifier);

      notifier.startNavigation(const LatLng(1, 2));
      expect(container.read(navigationStateProvider).isActive, isTrue);

      notifier.stopNavigation();
      final state = container.read(navigationStateProvider);
      expect(state.isActive, isFalse);
      expect(state.target, isNull);
    });

    test('stopNavigation disables followMode — user explicitly ending '
        'navigation also stops the camera from tracking', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      final notifier = container.read(navigationStateProvider.notifier);

      notifier.startNavigation(const LatLng(1, 2));
      expect(container.read(followModeProvider), FollowMode.active);

      notifier.stopNavigation();
      expect(container.read(followModeProvider), FollowMode.off);
    });

    test('starting a second navigation replaces the target', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      final notifier = container.read(navigationStateProvider.notifier);

      notifier.startNavigation(const LatLng(1, 2));
      notifier.startNavigation(const LatLng(3, 4));

      final state = container.read(navigationStateProvider);
      expect(state.target, const LatLng(3, 4));
      expect(state.isActive, isTrue);
    });
  });
}
