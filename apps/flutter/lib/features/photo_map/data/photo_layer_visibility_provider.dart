import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Whether the photo-locations layer is shown on the map. Defaults to off:
/// the layer triggers an OS photo-access prompt, so it must be opt-in.
/// Mirrors the persistence pattern of `markersVisibleProvider`.
final photoLayerVisibleProvider =
    NotifierProvider<PhotoLayerVisibleNotifier, bool>(
  PhotoLayerVisibleNotifier.new,
);

class PhotoLayerVisibleNotifier extends Notifier<bool> {
  static const _prefsKey = 'photoLayerVisible';

  @override
  bool build() {
    _loadFromPrefs();
    return false;
  }

  Future<void> _loadFromPrefs() async {
    final prefs = await SharedPreferences.getInstance();
    final value = prefs.getBool(_prefsKey);
    if (value != null) {
      state = value;
    }
  }

  void toggle() async {
    state = !state;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_prefsKey, state);
  }
}
