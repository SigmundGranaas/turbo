import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/journey/api.dart';
import 'package:turbo/features/navigation/data/navigation_state.dart';

/// Point-to-point navigation, now a thin projection over the unified
/// [activeJourneyProvider]. The journey owns follow-mode and the live state;
/// this provider preserves the existing `NavigationState` surface (target +
/// isActive) so callers and the nav chip/layers keep working unchanged.
final navigationStateProvider =
    NotifierProvider<NavigationStateNotifier, NavigationState>(
  NavigationStateNotifier.new,
);

class NavigationStateNotifier extends Notifier<NavigationState> {
  @override
  NavigationState build() {
    final journey = ref.watch(activeJourneyProvider);
    if (journey.kind == JourneyKind.navigatingToPoint &&
        journey.target != null) {
      return NavigationState(target: journey.target, isActive: true);
    }
    return NavigationState.inactive;
  }

  void startNavigation(LatLng target) =>
      ref.read(activeJourneyProvider.notifier).navigateToPoint(target);

  void stopNavigation() => ref.read(activeJourneyProvider.notifier).stop();
}
