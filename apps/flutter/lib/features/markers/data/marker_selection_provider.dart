import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Tracks which marker UUIDs the user has multi-selected for bulk actions
/// (delete, export). Empty set == no active selection mode.
final markerSelectionProvider =
    NotifierProvider<MarkerSelectionNotifier, Set<String>>(
        MarkerSelectionNotifier.new);

class MarkerSelectionNotifier extends Notifier<Set<String>> {
  @override
  Set<String> build() => const <String>{};

  bool contains(String uuid) => state.contains(uuid);

  bool get isActive => state.isNotEmpty;

  int get count => state.length;

  void toggle(String uuid) {
    final next = Set<String>.from(state);
    if (!next.add(uuid)) next.remove(uuid);
    state = next;
  }

  void add(String uuid) {
    if (state.contains(uuid)) return;
    state = {...state, uuid};
  }

  void remove(String uuid) {
    if (!state.contains(uuid)) return;
    final next = Set<String>.from(state)..remove(uuid);
    state = next;
  }

  void clear() {
    if (state.isEmpty) return;
    state = const <String>{};
  }
}
