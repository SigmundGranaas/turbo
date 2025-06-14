import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'dart:async';

import 'package:turbo/l10n/app_localizations.dart';
import '../../data/auth/auth_providers.dart';

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
    if (kDebugMode) {
      print('Processing Google auth callback');
    }

    try {
      final uri = Uri.base;
      final code = uri.queryParameters['code'];

      if (kDebugMode) {
        print('Auth code from URL: ${code?.substring(0, 10)}...');
      }

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
      if (kDebugMode) {
        print('Error processing auth callback: $e');
      }

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
                style: const TextStyle(fontSize: 18),
              ),
              const SizedBox(height: 24),
              if (_processingComplete && !_isProcessing) ...[
                ElevatedButton(
                  onPressed: () {
                    Navigator.of(context).pushReplacementNamed('/');
                  },
                  child: Text(l10n.continueToApp),
                ),
              ] else if (!_isProcessing) ...[
                ElevatedButton(
                  onPressed: () {
                    Navigator.of(context).pushReplacementNamed('/login');
                  },
                  child: Text(l10n.returnToLogin),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}