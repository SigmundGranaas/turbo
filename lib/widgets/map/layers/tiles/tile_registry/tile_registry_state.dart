import 'tile_provider.dart';

class TileRegistryState {
  final String? selectedGlobalId;
  final List<String> activeLocalIds;
  final List<String> activeOverlayIds;
  final Map<String, TileProviderWrapper> availableProviders;

  const TileRegistryState({
    required this.selectedGlobalId,
    required this.activeLocalIds,
    required this.activeOverlayIds,
    required this.availableProviders,
  });

  TileRegistryState copyWith({
    String? selectedGlobalId,
    List<String>? activeLocalIds,
    List<String>? activeOverlayIds,
    Map<String, TileProviderWrapper>? availableProviders,
  }) {
    return TileRegistryState(
      selectedGlobalId: selectedGlobalId ?? this.selectedGlobalId,
      activeLocalIds: activeLocalIds ?? this.activeLocalIds,
      activeOverlayIds: activeOverlayIds ?? this.activeOverlayIds,
      availableProviders: availableProviders ?? this.availableProviders,
    );
  }
}