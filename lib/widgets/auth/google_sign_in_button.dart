import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_sign_in/google_sign_in.dart';
import 'package:turbo/data/env_config.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:url_launcher/url_launcher.dart';
import '../../data/auth/auth_providers.dart';

class GoogleSignInButton extends ConsumerWidget {
  final bool isLoading;
  final VoidCallback onSignInStarted;
  final VoidCallback onSignInCompleted;

  const GoogleSignInButton({
    super.key,
    this.isLoading = false,
    required this.onSignInStarted,
    required this.onSignInCompleted,
  });

  Future<void> _handleGoogleSignIn(BuildContext context, WidgetRef ref) async {
    onSignInStarted();
    try {
      if (kIsWeb) {
        // Web flow: Redirect to the server's auth URL
        final l10n = context.l10n;
        final authUrl = await ref.read(authStateProvider.notifier).getGoogleAuthUrl();
        final uri = Uri.parse(authUrl);

        if (await canLaunchUrl(uri)) {
          await launchUrl(uri, webOnlyWindowName: '_self');
        } else {
          throw Exception(l10n.errorCouldNotLaunchUrl);
        }
      } else {
        // Native Android/iOS flow
        final googleSignIn = GoogleSignIn(
          serverClientId: EnvironmentConfig.googleServerClientId,
          scopes: ['email', 'profile'],
        );

        // Sign out from any previous session to ensure a fresh sign-in attempt
        await googleSignIn.signOut();

        final googleUser = await googleSignIn.signIn();

        if (googleUser == null) {
          // User canceled the sign-in
          if (kDebugMode) print('Google Sign-In canceled by user.');
          onSignInCompleted();
          return;
        }

        final googleAuth = await googleUser.authentication;
        final serverAuthCode = googleAuth.serverAuthCode;

        if (serverAuthCode == null) {
          throw Exception('Failed to get server auth code from Google.');
        }

        if (kDebugMode) {
          print('Native Google Sign-In successful. Sending serverAuthCode to backend.');
        }
        // The auth notifier will handle the API call and update the state.
        // It will set the loading state internally, so we don't need to await here.
        // The UI will update automatically when the auth state changes.
        ref.read(authStateProvider.notifier).processOAuthCallback(serverAuthCode);
      }
    } catch (e) {
      if (kDebugMode) print('Google sign-in error: $e');
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(context.l10n.signInFailed(e.toString()))),
        );
      }
    } finally {
      // We call onSignInCompleted in both cases to ensure the button's loading state resets.
      // For web, it's called immediately after launchUrl.
      // For native, it's called after the sign-in attempt (success or fail).
      onSignInCompleted();
    }
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return SizedBox(
      width: double.infinity,
      child: ElevatedButton.icon(
        onPressed: isLoading ? null : () => _handleGoogleSignIn(context, ref),
        icon: isLoading
            ? SizedBox(
          height: 24,
          width: 24,
          child: CircularProgressIndicator(
            strokeWidth: 2.5,
            color: colorScheme.primary,
          ),
        )
            : Image.asset(
          'assets/images/google_icon.webp',
          height: 24,
          width: 24,
        ),
        label: Text(
          l10n.signInWithGoogle,
          style: textTheme.labelLarge?.copyWith(
            color: colorScheme.onSurface,
            fontWeight: FontWeight.w500,
          ),
        ),
        style: ElevatedButton.styleFrom(
          foregroundColor: colorScheme.primary,
          backgroundColor: colorScheme.surface,
          disabledBackgroundColor: colorScheme.surface,
          elevation: 0,
          side: BorderSide(color: colorScheme.outline.withValues(alpha: 0.5)),
          padding: const EdgeInsets.symmetric(vertical: 18, horizontal: 24),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(28),
          ),
        ),
      ),
    );
  }
}