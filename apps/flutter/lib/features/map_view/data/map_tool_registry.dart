import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/map_tool.dart';

/// Composition root for map tools. Each tool feature exports a
/// [MapToolDescriptor] and registers it here at app startup; the map host
/// iterates the registry and never names a specific tool.
class MapToolRegistry {
  final Map<String, MapToolDescriptor> _byId;

  MapToolRegistry(Iterable<MapToolDescriptor> tools)
      : _byId = {for (final t in tools) t.id: t};

  MapToolDescriptor? get(String id) => _byId[id];
  Iterable<MapToolDescriptor> get all => _byId.values;
}

/// App-wide registry. The host wiring (`app/main.dart`) overrides this with the
/// list of tool descriptors the build ships.
final mapToolRegistryProvider = Provider<MapToolRegistry>((ref) {
  return MapToolRegistry(const []);
});

/// The tool currently mounted on the map, or null. Exactly one at a time —
/// this enforces the mutual exclusion the old per-screen tools never had.
final activeMapToolProvider =
    NotifierProvider<ActiveMapToolNotifier, String?>(ActiveMapToolNotifier.new);

class ActiveMapToolNotifier extends Notifier<String?> {
  @override
  String? build() => null;

  void activate(String id) => state = id;
  void deactivate() => state = null;
  void toggle(String id) => state = state == id ? null : id;
}
