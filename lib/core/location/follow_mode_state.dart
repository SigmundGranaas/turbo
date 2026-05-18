import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Three-state follow mode:
///   * [off]    — map is free; no snapping.
///   * [active] — map snaps to the user's fix continuously.
///   * [paused] — user dragged the map while following. Snap is suspended
///                so the user can look around without losing their intent;
///                tapping the location button resumes [active]. The explicit
///                close action on the mode chip transitions to [off].
enum FollowMode {
  off,
  active,
  paused;

  bool get isOn => this == FollowMode.active;
  bool get isOnOrPaused => this != FollowMode.off;
}

final followModeProvider = NotifierProvider<FollowModeNotifier, FollowMode>(
  FollowModeNotifier.new,
);

class FollowModeNotifier extends Notifier<FollowMode> {
  @override
  FollowMode build() => FollowMode.off;

  /// Engage active snapping. Used when the user opts in via the LocationButton,
  /// the long-press toggle, or a feature that wants to track the user
  /// (e.g. recording).
  void enable() => state = FollowMode.active;

  /// Fully exit follow mode. The ModeChip's close button + the long-press
  /// toggle's "off" position both come here.
  void disable() => state = FollowMode.off;

  /// Soft-deactivate. Called automatically when the user manually drags the
  /// map while following — keeps the intent visible (UI shows "paused")
  /// instead of silently flipping off.
  void pause() {
    if (state == FollowMode.active) state = FollowMode.paused;
  }

  /// Re-engage snapping from a paused state. Idempotent — calling on `off`
  /// has no effect; the LocationButton's "if (off) enable, if (paused) resume"
  /// branch handles that distinction.
  void resume() {
    if (state == FollowMode.paused) state = FollowMode.active;
  }

  /// Toggles between off and active. Paused counts as "on enough" — toggling
  /// from paused turns the feature off so the user can disengage with a
  /// single switch tap.
  void toggle() {
    state = state == FollowMode.off ? FollowMode.active : FollowMode.off;
  }
}
