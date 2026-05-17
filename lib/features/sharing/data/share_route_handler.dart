import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';

import '../../../core/sharing/shareable_link_codec.dart';
import 'pending_share_provider.dart';

/// Decodes incoming share URLs and pushes their payload onto the
/// [pendingShareProvider] so the map UI can react.
///
/// Used in three places:
///   * Web cold-start: parse `Uri.base` at app startup.
///   * Mobile cold-start: parse the initial app-link.
///   * Subsequent links (mobile): from the `app_links` stream.
class ShareRouteHandler {
  static final _log = Logger('ShareRouteHandler');

  final ProviderContainer _container;
  ShareRouteHandler(this._container);

  /// Returns true if [uri] was a share URL that was successfully decoded
  /// and pushed onto the pending provider.
  bool handle(Uri uri) {
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
}
