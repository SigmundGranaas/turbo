import 'dart:async';

import 'package:flutter_map/flutter_map.dart' show LatLngBounds;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../models/vector_feature.dart';
import '../models/vector_layer_source.dart';
import 'vector_layer_repository.dart';

/// Notifier holding the currently-visible vector features for a single source.
/// Keyed on the [VectorLayerSource.id] so each layer (trails, MetAlerts, …)
/// has independent state.
final viewportVectorFeaturesProvider = AsyncNotifierProvider.family<
    ViewportVectorFeaturesNotifier, List<VectorFeature>, String>(
  ViewportVectorFeaturesNotifier.new,
);

class ViewportVectorFeaturesNotifier
    extends AsyncNotifier<List<VectorFeature>> {
  ViewportVectorFeaturesNotifier(this.sourceId);

  final String sourceId;
  Timer? _debounce;
  LatLngBounds? _lastRequestedBounds;
  VectorLayerSource? _source;

  @override
  Future<List<VectorFeature>> build() async {
    ref.onDispose(() => _debounce?.cancel());
    return const [];
  }

  /// Registers the source description. Idempotent.
  void setSource(VectorLayerSource source) {
    _source = source;
  }

  /// Debounced refresh: schedules a new fetch ~350ms after the last
  /// invocation to coalesce a panning gesture into a single network call.
  void requestBounds(LatLngBounds bounds) {
    _lastRequestedBounds = bounds;
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 350), _executePending);
  }

  Future<void> _executePending() async {
    final bounds = _lastRequestedBounds;
    final source = _source;
    if (bounds == null || source == null) return;
    final repo = ref.read(vectorLayerRepositoryProvider);
    final features = await repo.featuresInBounds(
      source,
      bounds.southWest.latitude,
      bounds.southWest.longitude,
      bounds.northEast.latitude,
      bounds.northEast.longitude,
    );
    state = AsyncValue.data(features);
  }
}
