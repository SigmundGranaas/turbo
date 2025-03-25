import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_sign_in/google_sign_in.dart';
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
      if (kIsWeb) {
        await _webGoogleSignIn();
      } else {
        await _nativeGoogleSignIn();
      }
    } catch (e) {
      if (kDebugMode) {
        print('Google sign-in error: $e');
      }

      if(mounted){
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Sign in failed: ${e.toString()}')),
        );
      }



      widget.onSignInCompleted();
    }
  }

  Future<void> _webGoogleSignIn() async {
    try {
      // For web, we'll get the auth URL from the backend and redirect to it
      final authUrl = await ref.read(authStateProvider.notifier).getGoogleAuthUrl();

      if (kDebugMode) {
        print('Got Google auth URL: $authUrl');
      }

      // Launch the URL using the _self target to do a full page redirect
      final Uri uri = Uri.parse(authUrl);
      if (await canLaunchUrl(uri)) {
        await launchUrl(
          uri,
          mode: LaunchMode.externalApplication,
          webOnlyWindowName: '_self', // Forces a full page redirect instead of a popup
        );
      } else {
        throw Exception('Could not launch Google auth URL');
      }

      // The flow will continue in GoogleAuthCallbackPage after redirect
    } catch (e) {
      if (kDebugMode) {
        print('Web Google sign-in error: $e');
      }
      rethrow;
    }
  }

  Future<void> _nativeGoogleSignIn() async {
    try {
      final GoogleSignIn googleSignIn = GoogleSignIn(
        scopes: ['email', 'profile', 'openid'],
      );

      // Force a fresh sign-in experience
      await googleSignIn.signOut();

      final GoogleSignInAccount? account = await googleSignIn.signIn();

      if (account == null) {
        // User canceled sign-in
        widget.onSignInCompleted(); // Reset loading state
        return;
      }

      final GoogleSignInAuthentication auth = await account.authentication;

      if (auth.idToken == null) {
        throw Exception('Failed to get ID token from Google');
      }

      // Send the ID token to your backend
      await ref.read(authStateProvider.notifier).loginWithGoogle(auth.idToken!);

      widget.onSignInCompleted();
    } catch (e) {
      if (kDebugMode) {
        print('Native Google sign-in error: $e');
      }
      rethrow;
    }
  }
}