import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/core/location/follow_mode_state.dart';
import 'package:turbo/features/navigation/data/navigation_state.dart';

final navigationStateProvider =
    NotifierProvider<NavigationStateNotifier, NavigationState>(
  NavigationStateNotifier.new,
);

class NavigationStateNotifier extends Notifier<NavigationState> {
  @override
  NavigationState build() => NavigationState.inactive;

  void startNavigation(LatLng target) {
    state = NavigationState(target: target, isActive: true);
    ref.read(followModeProvider.notifier).enable();
  }

  void stopNavigation() {
    state = NavigationState.inactive;
  }
}
