import 'package:app_links/app_links.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';

import 'share_route_handler.dart';

final _log = Logger('ShareLinkListener');

/// Subscribes to subsequent app links (mobile only) and forwards any share
/// URLs to [ShareRouteHandler]. The initial cold-start URL is handled
/// directly in `main.dart` via `Uri.base`.
final shareLinkListenerProvider = Provider<void>((ref) {
  if (kIsWeb) return;

  try {
    final appLinks = AppLinks();
    final sub = appLinks.uriLinkStream.listen((uri) {
      ShareRouteHandler(ref.container).handle(uri);
    });
    ref.onDispose(sub.cancel);
  } catch (e, st) {
    _log.warning('Failed to subscribe to app links for share URLs', e, st);
  }
});
