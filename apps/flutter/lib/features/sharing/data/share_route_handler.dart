import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';

import '../../../core/sharing/shareable_link_codec.dart';
import 'pending_link_redemption_provider.dart';
import 'pending_share_provider.dart';

/// Decodes incoming share URLs and pushes their payload onto the
/// matching pending provider so the map UI / redemption listener can
/// react.
///
/// Recognized URL shapes:
///   * `/share/m?d=...`   — stateless marker payload (legacy)
///   * `/share/p?d=...`   — stateless path payload (legacy)
///   * `/share/r/<token>` — tracked link grant; redeemed server-side
///     once the user is authenticated.
///
/// Used in three places:
///   * Web cold-start: parse `Uri.base` at app startup.
///   * Mobile cold-start: parse the initial app-link.
///   * Subsequent links (mobile): from the `app_links` stream.
class ShareRouteHandler {
  static final _log = Logger('ShareRouteHandler');

  final ProviderContainer _container;
  ShareRouteHandler(this._container);

  /// Returns true if [uri] was a recognised share URL that was pushed
  /// onto one of the pending providers.
  bool handle(Uri uri) {
    final token = _extractLinkToken(uri);
    if (token != null) {
      _container.read(pendingLinkRedemptionProvider.notifier).push(token);
      return true;
    }
    try {
      final payload = ShareableLinkCodec.decodeShareUrl(uri);
      if (payload == null) return false;
      _container.read(pendingShareProvider.notifier).push(payload);
      return true;
    } catch (e, st) {
      _log.warning('Failed to decode share URL: $uri', e, st);
      return false;
    }
  }

  /// Returns the token from `/share/r/<token>` URLs, or null if [uri]
  /// is not in that shape.
  static String? _extractLinkToken(Uri uri) {
    final segs = uri.pathSegments.where((s) => s.isNotEmpty).toList();
    if (segs.length < 3) return null;
    final tail = segs.sublist(segs.length - 3);
    if (tail[0] != 'share' || tail[1] != 'r') return null;
    final token = tail[2];
    return token.isEmpty ? null : token;
  }
}
