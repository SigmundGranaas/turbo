import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/auth/auth_providers.dart';
import 'package:turbo/widgets/auth/register_screen.dart';

import 'login_view_desktop.dart';
import 'login_view_mobile.dart';

class LoginScreen extends ConsumerStatefulWidget {
  const LoginScreen({super.key});

  /// Shows the login screen as a responsive dialog or a full-screen page.
  static Future<void> show(BuildContext context) {
    // Standard breakpoint for mobile/desktop layouts
    final isDesktop = MediaQuery.of(context).size.width > 768;

    if (isDesktop) {
      return showDialog(
        context: context,
        builder: (context) => const Dialog(
          backgroundColor: Colors.transparent,
          surfaceTintColor: Colors.transparent,
          elevation: 0,
          insetPadding: EdgeInsets.all(16),
          child: SizedBox(
            width: 420,
            child: LoginScreen(),
          ),
        ),
      );
    } else {
      return Navigator.of(context).push(
        MaterialPageRoute(
          fullscreenDialog: true,
          builder: (context) => const LoginScreen(),
        ),
      );
    }
  }

  @override
  ConsumerState<LoginScreen> createState() => _LoginScreenState();
}

class _LoginScreenState extends ConsumerState<LoginScreen> {
  final _formKey = GlobalKey<FormState>();
  final _emailController = TextEditingController();
  final _passwordController = TextEditingController();

  // Separate loading states for different actions
  final _isLoading = StateProvider<bool>((ref) => false);
  final _isGoogleLoading = StateProvider<bool>((ref) => false);

  @override
  void initState() {
    super.initState();
    _setupListeners();
  }

  void _setupListeners() {
    // Listener to clear errors when user starts typing
    void clearErrors() => ref.read(authStateProvider.notifier).clearErrors();
    _emailController.addListener(clearErrors);
    _passwordController.addListener(clearErrors);

    // Listener to handle auth state changes (e.g., success, error)
    ref.listenManual<AuthState>(authStateProvider, (previous, next) {
      // On successful authentication, close the screen
      if (next.status == AuthStatus.authenticated) {
        if (kDebugMode) print("Login successful, closing screen.");
        if (Navigator.of(context).canPop()) {
          Navigator.of(context).pop();
        }
      }
      // Optional: Show a snackbar on specific error transitions
      if (previous?.status == AuthStatus.loading && next.status == AuthStatus.unauthenticated && next.errorMessage != null) {
        // You could show a snackbar here if desired, but the inline error is often better for forms.
      }
    });
  }

  @override
  void dispose() {
    _emailController.dispose();
    _passwordController.dispose();
    super.dispose();
  }

  Future<void> _login() async {
    // Hide keyboard
    FocusScope.of(context).unfocus();
    if (_formKey.currentState?.validate() ?? false) {
      ref.read(_isLoading.notifier).state = true;
      try {
        await ref.read(authStateProvider.notifier).login(
          _emailController.text.trim(),
          _passwordController.text,
        );
      } finally {
        if (mounted) {
          ref.read(_isLoading.notifier).state = false;
        }
      }
    }
  }

  void _navigateToRegister() {
    // Pop the current screen before pushing the new one
    if (Navigator.of(context).canPop()) {
      Navigator.of(context).pop();
    }
    RegisterScreen.show(context);
  }

  @override
  Widget build(BuildContext context) {
    final isDesktop = MediaQuery.of(context).size.width > 768;

    if (isDesktop) {
      return LoginViewDesktop(
        formKey: _formKey,
        emailController: _emailController,
        passwordController: _passwordController,
        onLogin: _login,
        onNavigateToRegister: _navigateToRegister,
        isLoadingProvider: _isLoading,
        isGoogleLoadingProvider: _isGoogleLoading,
      );
    } else {
      return LoginViewMobile(
        formKey: _formKey,
        emailController: _emailController,
        passwordController: _passwordController,
        onLogin: _login,
        onNavigateToRegister: _navigateToRegister,
        isLoadingProvider: _isLoading,
        isGoogleLoadingProvider: _isGoogleLoading,
      );
    }
  }
}