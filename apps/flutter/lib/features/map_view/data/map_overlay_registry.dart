import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/map_overlay.dart';

/// Composition root for persistent map overlays.
class MapOverlayRegistry {
  final List<MapOverlayDescriptor> _all;

  MapOverlayRegistry(Iterable<MapOverlayDescriptor> descriptors)
      : _all = descriptors.toList(growable: false);

  /// Descriptors in a slot, sorted so the highest priority comes first.
  List<MapOverlayDescriptor> inSlot(MapOverlaySlot slot) {
    final list = _all.where((d) => d.slot == slot).toList()
      ..sort((a, b) => b.priority.compareTo(a.priority));
    return list;
  }
}

/// App-wide registry. `app/main.dart` overrides this with the overlays the
/// build ships.
final mapOverlayRegistryProvider = Provider<MapOverlayRegistry>((ref) {
  return MapOverlayRegistry(const []);
});
