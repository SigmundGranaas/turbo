import 'package:flutter_map/flutter_map.dart';
import 'package:map_app/widgets/map/layers/tiles/tile_registry/tile_registry_state.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import 'tile_provider.dart';

part 'tile_registry.g.dart';

@riverpod
class TileRegistry extends _$TileRegistry {
  @override
  TileRegistryState build() {
    return const TileRegistryState(
      activeGlobalIds: [],
      activeLocalIds: [],
      activeOverlayIds: [],
      availableProviders: {},
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

@riverpod
List<TileProviderWrapper> globalLayers(GlobalLayersRef ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((provider) => provider.category == TileCategory.global)
      .toList();
}

@riverpod
List<TileProviderWrapper> localLayers(LocalLayersRef ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((provider) => provider.category == TileCategory.local)
      .toList();
}

@riverpod
List<TileProviderWrapper> overlayLayers(OverlayLayersRef ref) {
  final registry = ref.watch(tileRegistryProvider);
  return registry.availableProviders.values
      .where((provider) => provider.category == TileCategory.overlay)
      .toList();
}
