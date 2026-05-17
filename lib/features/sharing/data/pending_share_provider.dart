import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/sharing/shareable_link_codec.dart';

/// Holds at most one [SharedPayload] that arrived via a share URL and is
/// waiting for the map UI to consume it. Cleared once the preview sheet
/// opens.
final pendingShareProvider =
    NotifierProvider<PendingShareNotifier, SharedPayload?>(
  PendingShareNotifier.new,
);

class PendingShareNotifier extends Notifier<SharedPayload?> {
  @override
  SharedPayload? build() => null;

  void push(SharedPayload payload) {
    state = payload;
  }

  /// Returns the current payload (if any) and clears the state in one step.
  SharedPayload? consume() {
    final current = state;
    state = null;
    return current;
  }
}
