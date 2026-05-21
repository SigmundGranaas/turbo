import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

final collectionVisibilityProvider =
    NotifierProvider<CollectionVisibilityNotifier, Map<String, bool>>(
  CollectionVisibilityNotifier.new,
);

class CollectionVisibilityNotifier extends Notifier<Map<String, bool>> {
  static const _prefsKey = 'collectionVisibility';

  @override
  Map<String, bool> build() {
    _loadFromPrefs();
    return <String, bool>{};
  }

  Future<void> _loadFromPrefs() async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_prefsKey);
    if (raw == null) return;
    try {
      final decoded = jsonDecode(raw) as Map<String, dynamic>;
      state = decoded.map((k, v) => MapEntry(k, v == true));
    } catch (_) {
      // Ignore malformed prefs; treat as empty.
    }
  }

  Future<void> _persist() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_prefsKey, jsonEncode(state));
  }

  bool isVisible(String collectionUuid) {
    return state[collectionUuid] ?? true;
  }

  void toggle(String collectionUuid) {
    setVisible(collectionUuid, !isVisible(collectionUuid));
  }

  void setVisible(String collectionUuid, bool visible) {
    final next = Map<String, bool>.from(state);
    next[collectionUuid] = visible;
    state = next;
    _persist();
  }

  void clearFor(String collectionUuid) {
    if (!state.containsKey(collectionUuid)) return;
    final next = Map<String, bool>.from(state);
    next.remove(collectionUuid);
    state = next;
    _persist();
  }
}
