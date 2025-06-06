import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:url_launcher/url_launcher.dart';
import '../../data/auth/auth_providers.dart';

class GoogleSignInButton extends ConsumerStatefulWidget {
  final bool isLoading;
  final VoidCallback onSignInStarted;
  final VoidCallback onSignInCompleted;

  const GoogleSignInButton({
    super.key,
    this.isLoading = false,
    required this.onSignInStarted,
    required this.onSignInCompleted,
  });

  @override
  ConsumerState<GoogleSignInButton> createState() => _GoogleSignInButtonState();
}

class _GoogleSignInButtonState extends ConsumerState<GoogleSignInButton> {
  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return SizedBox(
      width: double.infinity,
      child: ElevatedButton(
        onPressed: widget.isLoading ? null : _handleGoogleSignIn,
        style: ElevatedButton.styleFrom(
          foregroundColor: colorScheme.onSurface,
          backgroundColor: colorScheme.surface,
          elevation: 0,
          side: BorderSide(color: colorScheme.outline.withValues(alpha: 0.5)),
          padding: const EdgeInsets.symmetric(vertical: 18, horizontal: 24),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(28),
          ),
        ),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            if (widget.isLoading)
              SizedBox(
                height: 18,
                width: 18,
                child: CircularProgressIndicator(
                  strokeWidth: 2,
                  color: colorScheme.primary,
                ),
              )
            else
              Image.asset(
                'assets/images/google_icon.webp',
                height: 24,
                width: 24,
              ),
            const SizedBox(width: 12),
            Text(
              'Sign in with Google',
              style: textTheme.labelLarge?.copyWith(
                fontWeight: FontWeight.w500,
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _handleGoogleSignIn() async {
    widget.onSignInStarted();

    try {
      final authUrl = await ref.read(authStateProvider.notifier).getGoogleAuthUrl();
      final uri = Uri.parse(authUrl);

      if (kDebugMode) {
        print('Launching Google auth URL: $authUrl');
      }

      if (await canLaunchUrl(uri)) {
        // On web, this performs a full page redirect. The app will reload on the
        // callback URL and `initializeAndHandleInitialLink` will process it.
        // On mobile, this opens the user's default browser. The app's deep link
        // listener will catch the redirect and `linkStreamHandlerProvider` will process it.
        await launchUrl(
          uri,
          mode: LaunchMode.externalApplication,
          // This is critical for the web redirect flow to work.
          webOnlyWindowName: '_self',
        );
      } else {
        throw Exception('Could not launch Google auth URL');
      }

      // For web, the sign-in is not "completed" here as a redirect happens.
      // For mobile, the user is now in the browser, so we can complete the "loading" state.
      if (!kIsWeb) {
        widget.onSignInCompleted();
      }
    } catch (e) {
      if (kDebugMode) {
        print('Google sign-in error: $e');
      }
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Sign in failed: ${e.toString()}')),
        );
      }
      widget.onSignInCompleted();
    }
  }
}