import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/layer_preference_service.dart';
import 'package:turbo/widgets/map/layers/tiles/tile_registry/tile_registry_state.dart';

import 'tile_provider.dart';

// FIX: Changed to NotifierProvider to make it persistent.
final tileRegistryProvider =
NotifierProvider<TileRegistry, TileRegistryState>(
  TileRegistry.new,
);

class TileRegistry extends Notifier<TileRegistryState> {
  @override
  TileRegistryState build() {
    return const TileRegistryState(
      activeGlobalIds: [],
      activeLocalIds: [],
      activeOverlayIds: [],
      availableProviders: {},
    );
  }

  void _persistState() {
    ref.read(layerPreferenceServiceProvider).saveLayers(
      global: state.activeGlobalIds,
      local: state.activeLocalIds,
      overlays: state.activeOverlayIds,
    );
  }

  void registerProvider(TileProviderWrapper provider) {
    final updatedProviders = Map<String, TileProviderWrapper>.from(state.availableProviders);
    updatedProviders[provider.id] = provider;

    state = state.copyWith(
      availableProviders: updatedProviders,
    );
  }

  void unregisterProvider(String providerId) {
    final updatedProviders = Map<String, TileProviderWrapper>.from(state.availableProviders)
      ..remove(providerId);

    state = state.copyWith(
      availableProviders: updatedProviders,
      activeGlobalIds:  state.activeGlobalIds.where((id) => id != providerId).toList(),
      activeLocalIds: state.activeLocalIds.where((id) => id != providerId).toList(),
      activeOverlayIds: state.activeOverlayIds.where((id) => id != providerId).toList(),
    );
  }

  void toggleGlobalLayer(String providerId) {
    if (state.availableProviders[providerId]?.category != TileCategory.global) {
      throw ArgumentError('Provider must be a global layer');
    }

    state = state.copyWith(
      activeGlobalIds: state.activeGlobalIds.contains(providerId)
          ? state.activeGlobalIds.where((id) => id != providerId).toList()
          : [...state.activeGlobalIds, providerId],
    );
    _persistState();
  }

  void toggleLocalLayer(String providerId) {
    if (state.availableProviders[providerId]?.category != TileCategory.local) {
      throw ArgumentError('Provider must be a local layer');
    }

    state = state.copyWith(
      activeLocalIds: state.activeLocalIds.contains(providerId)
          ? state.activeLocalIds.where((id) => id != providerId).toList()
          : [...state.activeLocalIds, providerId],
    );
    _persistState();
  }

  void toggleOverlay(String providerId) {
    if (state.availableProviders[providerId]?.category != TileCategory.overlay) {
      throw ArgumentError('Provider must be an overlay layer');
    }

    state = state.copyWith(
      activeOverlayIds: state.activeOverlayIds.contains(providerId)
          ? state.activeOverlayIds.where((id) => id != providerId).toList()
          : [...state.activeOverlayIds, providerId],
    );
    _persistState();
  }

  void initializeWith(List<String> global, List<String> local, List<String> overlays) {
    state = state.copyWith(
      activeGlobalIds: global,
      activeLocalIds: local,
      activeOverlayIds: overlays,
    );
  }

  List<TileLayer> getActiveLayers() {
    final layers = <TileLayer>[];

    // Add local layers
    layers.addAll(
        state.activeGlobalIds
            .map((id) => state.availableProviders[id]?.createTileLayer())
            .whereType<TileLayer>()
    );

    // Add local layers
    layers.addAll(
        state.activeLocalIds
            .map((id) => state.availableProviders[id]?.createTileLayer())
            .whereType<TileLayer>()
    );

    // Add overlays
    layers.addAll(
        state.activeOverlayIds
            .map((id) => state.availableProviders[id]?.createTileLayer())
            .whereType<TileLayer>()
    );

    return layers;
  }
}

// FIX: Changed to Provider to avoid auto-disposal.
final globalLayersProvider = Provider<List<TileProviderWrapper>>((ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((provider) => provider.category == TileCategory.global)
      .toList();
});

// FIX: Changed to Provider to avoid auto-disposal.
final localLayersProvider = Provider<List<TileProviderWrapper>>((ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((provider) => provider.category == TileCategory.local)
      .toList();
});

// FIX: Changed to Provider to avoid auto-disposal.
final overlayLayersProvider = Provider<List<TileProviderWrapper>>((ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((provider) => provider.category == TileCategory.overlay)
      .toList();
});