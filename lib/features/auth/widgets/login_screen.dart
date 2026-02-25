import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../data/auth_providers.dart';
import 'package:turbo/core/theme/utils.dart';
import './register_screen.dart';

import 'login_view_desktop.dart';
import 'login_view_mobile.dart';

class LoginScreen extends ConsumerStatefulWidget {
  const LoginScreen({super.key});

  static Future<void> show(BuildContext context) {
    final isDesktop = MediaQuery.of(context).size.width > 768;

    if (isDesktop) {
      return showDialog(
        context: context,
        builder: (context) => Dialog(
          shape:
          RoundedRectangleBorder(borderRadius: BorderRadius.circular(28)),
          clipBehavior: Clip.antiAlias,
          child: const SizedBox(
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

  final _isLoading = NotifierProvider<LoadingNotifier, bool>(LoadingNotifier.new);
  final _isGoogleLoading = NotifierProvider<LoadingNotifier, bool>(LoadingNotifier.new);

  @override
  void initState() {
    super.initState();
    _setupListeners();
  }

  void _setupListeners() {
    void clearErrors() => ref.read(authStateProvider.notifier).clearErrors();
    _emailController.addListener(clearErrors);
    _passwordController.addListener(clearErrors);

    ref.listenManual<AuthState>(authStateProvider, (previous, next) {
      if (next.status == AuthStatus.authenticated) {
        if (kDebugMode) print("Login successful, closing screen.");
        if (Navigator.of(context).canPop()) {
          Navigator.of(context).pop();
        }
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
    FocusScope.of(context).unfocus();
    if (_formKey.currentState?.validate() ?? false) {
      ref.read(_isLoading.notifier).set(true);
      try {
        await ref.read(authStateProvider.notifier).login(
          _emailController.text.trim(),
          _passwordController.text,
        );
      } finally {
        if (mounted) {
          ref.read(_isLoading.notifier).set(false);
        }
      }
    }
  }

  void _navigateToRegister() {
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
