import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

final markersVisibleProvider = NotifierProvider<_MarkersVisibleNotifier, bool>(
  _MarkersVisibleNotifier.new,
);

final savedPathsVisibleProvider = NotifierProvider<_SavedPathsVisibleNotifier, bool>(
  _SavedPathsVisibleNotifier.new,
);

class _MarkersVisibleNotifier extends Notifier<bool> {
  static const _prefsKey = 'markersVisible';

  @override
  bool build() {
    _loadFromPrefs();
    return true;
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

class _SavedPathsVisibleNotifier extends Notifier<bool> {
  static const _prefsKey = 'savedPathsVisible';

  @override
  bool build() {
    _loadFromPrefs();
    return true;
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
