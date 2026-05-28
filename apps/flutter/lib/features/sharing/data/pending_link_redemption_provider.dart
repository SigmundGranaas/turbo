import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Holds a tracked link-share token captured from a deep-link before
/// the user finishes signing in. Once auth is established the
/// link-redemption listener pops the token off and hits the server
/// /api/sharing/grants/links/{token}/redeem endpoint.
///
/// Distinct from the stateless `pendingShareProvider` (which holds a
/// fully-decoded `SharedPayload` for legacy /share/m and /share/p
/// URLs) because tracked redemption needs a network round-trip and
/// an authenticated user.
class PendingLinkRedemptionNotifier extends Notifier<String?> {
  @override
  String? build() => null;

  void push(String token) => state = token;

  String? take() {
    final t = state;
    state = null;
    return t;
  }
}

final pendingLinkRedemptionProvider =
    NotifierProvider<PendingLinkRedemptionNotifier, String?>(
        PendingLinkRedemptionNotifier.new);
