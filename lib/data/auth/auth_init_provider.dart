import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:app_links/app_links.dart';

import 'auth_providers.dart';

// This provider is responsible for handling SUBSEQUENT deep links after app is running.
// The initial deep link is handled by AuthStateNotifier.initializeAndHandleInitialLink().
final linkStreamHandlerProvider = Provider<void>((ref) {
  // Skip setting up link stream on web platform
  if (kIsWeb) return;

  final authNotifier = ref.read(authStateProvider.notifier);

  // Listen for subsequent links (if app is already running)
  try {
    final appLinks = AppLinks();
    final sub = appLinks.uriLinkStream.listen((uri) {
      // The stream from app_links emits non-nullable Uri objects.
      handleDeepLinkForProvider(uri.toString(), authNotifier);
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
  // The native OAuth flow no longer uses deep links for the callback.
  // The previous check for `/oauth2callback` has been removed.
  // This function can be used for other deep link features in the future.
  // Example:
  // final uri = Uri.parse(link);
  // if (uri.path.contains('/marker/')) { ... }
}