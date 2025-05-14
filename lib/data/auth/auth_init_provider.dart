import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uni_links/uni_links.dart';

import 'auth_providers.dart';

// This provider is responsible for handling SUBSEQUENT deep links after app is running.
// The initial deep link is handled by AuthStateNotifier.initializeAndHandleInitialLink().
final linkStreamHandlerProvider = Provider<void>((ref) {
  // Skip setting up link stream on web platform
  if (kIsWeb) return;

  final authNotifier = ref.read(authStateProvider.notifier);

  // Listen for subsequent links (if app is already running)
  try {
    final sub = uriLinkStream.listen((uri) {
      if (uri != null) {
        handleDeepLinkForProvider(uri.toString(), authNotifier);
      }
    });

    // Cancel the subscription when the provider is disposed
    ref.onDispose(() => sub.cancel());
  } catch (e) {
    if (kDebugMode) {
      print('Error setting up link stream: $e');
    }
  }
});

void handleDeepLinkForProvider(String link, AuthStateNotifier authNotifier) {
  if (kDebugMode) {
    print('Deep Link Handler: Got deep link: $link');
  }
  final uri = Uri.parse(link);
  // Check if this is our OAuth callback for mobile
  if (uri.path.contains('/oauth2callback')) {
    final code = uri.queryParameters['code'];
    if (code != null) {
      authNotifier.processOAuthCallback(code);
    }
  }
}