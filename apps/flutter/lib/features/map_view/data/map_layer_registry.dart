import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/map_layer.dart';

/// Ordered composition root for map layers. Registry order == render order.
class MapLayerRegistry {
  final List<MapLayerDescriptor> _ordered;

  MapLayerRegistry(Iterable<MapLayerDescriptor> descriptors)
      : _ordered = descriptors.toList(growable: false);

  List<MapLayerDescriptor> get all => List.unmodifiable(_ordered);
}

/// App-wide registry. `app/main.dart` overrides this with the layer stack the
/// build ships, between the base tiles (host-owned) and the active tool's
/// layers (host-owned).
final mapLayerRegistryProvider = Provider<MapLayerRegistry>((ref) {
  return MapLayerRegistry(const []);
});
