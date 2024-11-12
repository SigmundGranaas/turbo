import 'tile_provider.dart';

class TileRegistryState {
  final  List<String> activeGlobalIds;
  final List<String> activeLocalIds;
  final List<String> activeOverlayIds;
  final Map<String, TileProviderWrapper> availableProviders;

  const TileRegistryState({
    required this.activeGlobalIds,
    required this.activeLocalIds,
    required this.activeOverlayIds,
    required this.availableProviders,
  });

  TileRegistryState copyWith({
    List<String>? activeGlobalIds,
    List<String>? activeLocalIds,
    List<String>? activeOverlayIds,
    Map<String, TileProviderWrapper>? availableProviders,
  }) {
    return TileRegistryState(
      activeGlobalIds: activeGlobalIds ?? this.activeGlobalIds,
      activeLocalIds: activeLocalIds ?? this.activeLocalIds,
      activeOverlayIds: activeOverlayIds ?? this.activeOverlayIds,
      availableProviders: availableProviders ?? this.availableProviders,
    );
  }
}