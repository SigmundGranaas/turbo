import 'package:flutter/foundation.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

@immutable
class TileRegistryState {
  final List<String> activeGlobalIds;
  final List<String> activeLocalIds;
  final List<String> activeOverlayIds;
  final List<String> activeOfflineIds;
  final Map<String, TileProviderConfig> availableProviders;

  const TileRegistryState({
    required this.activeGlobalIds,
    required this.activeLocalIds,
    required this.activeOverlayIds,
    required this.activeOfflineIds,
    required this.availableProviders,
  });

  TileRegistryState copyWith({
    List<String>? activeGlobalIds,
    List<String>? activeLocalIds,
    List<String>? activeOverlayIds,
    List<String>? activeOfflineIds,
    Map<String, TileProviderConfig>? availableProviders,
  }) {
    return TileRegistryState(
      activeGlobalIds: activeGlobalIds ?? this.activeGlobalIds,
      activeLocalIds: activeLocalIds ?? this.activeLocalIds,
      activeOverlayIds: activeOverlayIds ?? this.activeOverlayIds,
      activeOfflineIds: activeOfflineIds ?? this.activeOfflineIds,
      availableProviders: availableProviders ?? this.availableProviders,
    );
  }
}