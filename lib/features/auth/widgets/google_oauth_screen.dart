import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';
import 'dart:async';

import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import '../data/auth_providers.dart';

final _log = Logger('GoogleAuthCallback');

class GoogleAuthCallbackPage extends ConsumerStatefulWidget {
  const GoogleAuthCallbackPage({super.key});

  @override
  ConsumerState<GoogleAuthCallbackPage> createState() => _GoogleAuthCallbackPageState();
}

class _GoogleAuthCallbackPageState extends ConsumerState<GoogleAuthCallbackPage> {
  bool _isProcessing = true;
  String? _message;
  bool _processingComplete = false;
  Timer? _redirectTimer;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _processAuthCallback();
    });
  }

  @override
  void dispose() {
    _redirectTimer?.cancel();
    super.dispose();
  }

  Future<void> _processAuthCallback() async {
    final l10n = context.l10n;
    _log.fine('Processing Google auth callback');

    try {
      final uri = Uri.base;
      final code = uri.queryParameters['code'];

      _log.fine(() => 'Auth code from URL: ${code?.substring(0, 10)}...');

      if (code != null) {
        await ref.read(authStateProvider.notifier).processOAuthCallback(code);
        await Future.delayed(const Duration(milliseconds: 500));

        final authState = ref.read(authStateProvider);

        if (authState.status == AuthStatus.authenticated) {
          setState(() {
            _message = l10n.loginSuccessfulRedirecting;
            _isProcessing = false;
            _processingComplete = true;
          });

          _redirectTimer = Timer(const Duration(seconds: 2), () {
            if (mounted) {
              Navigator.of(context).pushReplacementNamed('/');
            }
          });
        } else {
          setState(() {
            _message = l10n.loginFailed(authState.errorMessage ?? 'Unknown error');
            _isProcessing = false;
            _processingComplete = true;
          });
        }
      } else {
        setState(() {
          _message = l10n.noAuthCodeFound;
          _isProcessing = false;
        });
      }
    } catch (e) {
      _log.warning('Error processing auth callback', e);

      setState(() {
        _message = l10n.errorProcessingLogin(e.toString());
        _isProcessing = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.googleAuth),
        automaticallyImplyLeading: !_isProcessing,
      ),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(24.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              if (_isProcessing)
                const CircularProgressIndicator(),
              const SizedBox(height: 24),
              Text(
                _message ?? l10n.processingGoogleLogin,
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.titleMedium,
              ),
              const SizedBox(height: 24),
              if (_processingComplete && !_isProcessing) ...[
                AppButton.primary(
                  text: l10n.continueToApp,
                  onPressed: () => Navigator.of(context).pushReplacementNamed('/'),
                ),
              ] else if (!_isProcessing) ...[
                AppButton.secondary(
                  text: l10n.returnToLogin,
                  onPressed: () => Navigator.of(context).pushReplacementNamed('/login'),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}