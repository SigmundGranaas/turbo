
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uni_links/uni_links.dart';

import 'auth_providers.dart';

/// Provider that handles initialization of auth state and deep links
final authInitializationProvider = FutureProvider<void>((ref) async {
  // First check if we have stored tokens
  await ref.read(authStateProvider.notifier).initialize();

  if (kIsWeb) {
    // For web, check if we're on a callback URL
    final uri = Uri.base;
    if (uri.path.contains('/login/callback')) {
      if (kDebugMode) {
        print('Detected callback URL: $uri');
      }

      // Extract any query parameters
      final code = uri.queryParameters['code'];

      if (code != null) {
        // Process the OAuth callback
        await ref.read(authStateProvider.notifier).processOAuthCallback(code);
      }
    }
  } else {
    // For mobile, listen for deep links
    try {
      final initialLink = await getInitialLink();
      if (initialLink != null) {
        _handleDeepLink(initialLink, ref);
      }

      // Listen for subsequent links (if app is already running)
      uriLinkStream.listen((uri) {
        if (uri != null) {
          _handleDeepLink(uri.toString(), ref);
        }
      });
    } catch (e) {
      if (kDebugMode) {
        print('Error handling deep links: $e');
      }
    }
  }
});

void _handleDeepLink(String link, Ref ref) {
  if (kDebugMode) {
    print('Got deep link: $link');
  }

  final uri = Uri.parse(link);

  // Check if this is our OAuth callback
  if (uri.path.contains('/oauth2callback')) {
    final code = uri.queryParameters['code'];

    if (code != null) {
      // Process the OAuth callback
      ref.read(authStateProvider.notifier).processOAuthCallback(code);
    }
  }
}