import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/map_view/models/map_view_state.dart';

/// Manages the state of the map's viewport (center, zoom).
/// It is the single source of truth for the map's position.
class MapViewStateNotifier extends StateNotifier<MapViewState> {
  MapViewStateNotifier() : super(MapViewState.initial());

  /// Updates the state based on a MapEvent.
  /// This should be called from the map's `onMapEvent` callback.
  void onMapEvent(MapEvent event) {
    if (event.camera.center != state.center ||
        event.camera.zoom != state.zoom) {
      state = state.copyWith(
        center: event.camera.center,
        zoom: event.camera.zoom,
      );
    }
  }
}