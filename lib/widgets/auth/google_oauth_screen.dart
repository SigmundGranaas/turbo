import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'dart:async';

import '../../data/auth/auth_providers.dart';

class GoogleAuthCallbackPage extends ConsumerStatefulWidget {
  const GoogleAuthCallbackPage({super.key});

  @override
  ConsumerState<GoogleAuthCallbackPage> createState() => _GoogleAuthCallbackPageState();
}

class _GoogleAuthCallbackPageState extends ConsumerState<GoogleAuthCallbackPage> {
  bool _isProcessing = true;
  String _message = "Processing Google login...";
  bool _processingComplete = false;
  Timer? _redirectTimer;

  @override
  void initState() {
    super.initState();
    // Use a post-frame callback to ensure the context is ready
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
    if (kDebugMode) {
      print('Processing Google auth callback');
    }

    try {
      // Get the current URL
      final uri = Uri.base;

      // Extract the code parameter
      final code = uri.queryParameters['code'];

      if (kDebugMode) {
        print('Auth code from URL: ${code?.substring(0, 10)}...');
      }

      if (code != null) {
        // Process the code
        await ref.read(authStateProvider.notifier).processOAuthCallback(code);

        // Wait a moment to let the state update
        await Future.delayed(const Duration(milliseconds: 500));

        // Check the authentication state
        final authState = ref.read(authStateProvider);

        if (authState.status == AuthStatus.authenticated) {
          setState(() {
            _message = "Login successful! Redirecting...";
            _isProcessing = false;
            _processingComplete = true;
          });

          // Auto-redirect after a delay
          _redirectTimer = Timer(const Duration(seconds: 2), () {
            if (mounted) {
              Navigator.of(context).pushReplacementNamed('/');
            }
          });
        } else {
          setState(() {
            _message = "Login failed: ${authState.errorMessage ?? 'Unknown error'}";
            _isProcessing = false;
            _processingComplete = true;
          });
        }
      } else {
        setState(() {
          _message = "No authorization code found in the URL";
          _isProcessing = false;
        });
      }
    } catch (e) {
      if (kDebugMode) {
        print('Error processing auth callback: $e');
      }

      setState(() {
        _message = "Error processing login: $e";
        _isProcessing = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Google Authentication'),
        automaticallyImplyLeading: !_isProcessing,
      ),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(24.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              if (_isProcessing)
                const CircularProgressIndicator(
                  valueColor: AlwaysStoppedAnimation<Color>(Colors.blue),
                ),
              const SizedBox(height: 24),
              Text(
                _message,
                textAlign: TextAlign.center,
                style: const TextStyle(fontSize: 18),
              ),
              const SizedBox(height: 24),
              if (_processingComplete && !_isProcessing) ...[
                ElevatedButton(
                  onPressed: () {
                    Navigator.of(context).pushReplacementNamed('/');
                  },
                  style: ElevatedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 12),
                  ),
                  child: const Text('Continue to App'),
                ),
              ] else if (!_isProcessing) ...[
                ElevatedButton(
                  onPressed: () {
                    Navigator.of(context).pushReplacementNamed('/login');
                  },
                  style: ElevatedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 12),
                  ),
                  child: const Text('Return to Login'),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}