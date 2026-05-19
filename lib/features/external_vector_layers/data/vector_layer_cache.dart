import 'dart:collection';

import '../models/vector_feature.dart';

/// In-memory LRU keyed on quantised bbox per source.
///
/// Caches by Slippy-Map-like tile coordinates derived from the requested
/// viewport. The fetcher debounces map events and snaps the request bbox to
/// the nearest tile grid at the configured zoom, so identical viewports
/// share an entry across pans.
class VectorLayerCache {
  final int capacity;

  VectorLayerCache({this.capacity = 64});

  final LinkedHashMap<String, List<VectorFeature>> _entries =
      LinkedHashMap<String, List<VectorFeature>>();

  /// Returns the cached payload for [key] and bumps its LRU position.
  List<VectorFeature>? get(String key) {
    final hit = _entries.remove(key);
    if (hit != null) _entries[key] = hit;
    return hit;
  }

  void put(String key, List<VectorFeature> features) {
    if (_entries.containsKey(key)) {
      _entries.remove(key);
    } else if (_entries.length >= capacity) {
      _entries.remove(_entries.keys.first);
    }
    _entries[key] = features;
  }

  void clear() => _entries.clear();

  int get size => _entries.length;
}
