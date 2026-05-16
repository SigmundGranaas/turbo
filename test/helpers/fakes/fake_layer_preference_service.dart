import 'package:turbo/features/tile_providers/data/layer_preference_service.dart';

/// In-memory replacement for [LayerPreferenceService]. Lets tests assert on
/// what the [TileRegistry] persisted without going through `SharedPreferences`.
///
/// Inject via:
/// ```dart
/// final fake = FakeLayerPreferenceService();
/// ProviderScope(overrides: [
///   layerPreferenceServiceProvider.overrideWithValue(fake),
/// ], ...);
/// ```
class FakeLayerPreferenceService implements LayerPreferenceService {
  List<String> global;
  List<String> local;
  List<String> overlays;
  List<String> offline;

  /// Number of times [saveLayers] has been called — useful for asserting
  /// persistence happened in response to a toggle.
  int saveCount = 0;

  FakeLayerPreferenceService({
    List<String>? initialGlobal,
    List<String>? initialLocal,
    List<String>? initialOverlays,
    List<String>? initialOffline,
  })  : global = List.of(initialGlobal ?? const []),
        local = List.of(initialLocal ?? const []),
        overlays = List.of(initialOverlays ?? const []),
        offline = List.of(initialOffline ?? const []);

  @override
  Future<void> saveLayers({
    required List<String> global,
    required List<String> local,
    required List<String> overlays,
    required List<String> offline,
  }) async {
    this.global = List.of(global);
    this.local = List.of(local);
    this.overlays = List.of(overlays);
    this.offline = List.of(offline);
    saveCount++;
  }

  @override
  Future<Map<String, List<String>>> getSavedLayers() async {
    return {
      'global': List.of(global),
      'local': List.of(local),
      'overlays': List.of(overlays),
      'offline': List.of(offline),
    };
  }
}
