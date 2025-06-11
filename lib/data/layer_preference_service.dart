import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

final layerPreferenceServiceProvider = Provider<LayerPreferenceService>((ref) {
  return LayerPreferenceService();
});

class LayerPreferenceService {
  static const _globalKey = 'active_global_layers';
  static const _localKey = 'active_local_layers';
  static const _overlayKey = 'active_overlay_layers';

  Future<void> saveLayers({
    required List<String> global,
    required List<String> local,
    required List<String> overlays,
  }) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setStringList(_globalKey, global);
    await prefs.setStringList(_localKey, local);
    await prefs.setStringList(_overlayKey, overlays);
  }

  Future<Map<String, List<String>>> getSavedLayers() async {
    final prefs = await SharedPreferences.getInstance();
    return {
      'global': prefs.getStringList(_globalKey) ?? [],
      'local': prefs.getStringList(_localKey) ?? [],
      'overlays': prefs.getStringList(_overlayKey) ?? [],
    };
  }
}