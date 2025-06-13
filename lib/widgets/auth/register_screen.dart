import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/auth/auth_providers.dart';

import 'login_screen.dart';
import 'register_view_desktop.dart';
import 'register_view_mobile.dart';


class RegisterScreen extends ConsumerStatefulWidget {
  const RegisterScreen({super.key});

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
            child: RegisterScreen(),
          ),
        ),
      );
    } else {
      return Navigator.of(context).push(
        MaterialPageRoute(
          fullscreenDialog: true,
          builder: (context) => const RegisterScreen(),
        ),
      );
    }
  }

  @override
  ConsumerState<RegisterScreen> createState() => _RegisterScreenState();
}

class _RegisterScreenState extends ConsumerState<RegisterScreen> {
  final _formKey = GlobalKey<FormState>();
  final _emailController = TextEditingController();
  final _passwordController = TextEditingController();

  final _isLoading = StateProvider<bool>((ref) => false);
  final _isGoogleLoading = StateProvider<bool>((ref) => false);

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
        if (kDebugMode) print("Registration successful, closing screen.");
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

  Future<void> _register() async {
    FocusScope.of(context).unfocus();
    if (_formKey.currentState?.validate() ?? false) {
      ref.read(_isLoading.notifier).state = true;
      try {
        await ref.read(authStateProvider.notifier).register(
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

  void _navigateToLogin() {
    if (Navigator.of(context).canPop()) {
      Navigator.of(context).pop();
    }
    LoginScreen.show(context);
  }

  @override
  Widget build(BuildContext context) {
    final isDesktop = MediaQuery.of(context).size.width > 768;

    if (isDesktop) {
      return RegisterViewDesktop(
        formKey: _formKey,
        emailController: _emailController,
        passwordController: _passwordController,
        onRegister: _register,
        onNavigateToLogin: _navigateToLogin,
        isLoadingProvider: _isLoading,
        isGoogleLoadingProvider: _isGoogleLoading,
      );
    } else {
      return RegisterViewMobile(
        formKey: _formKey,
        emailController: _emailController,
        passwordController: _passwordController,
        onRegister: _register,
        onNavigateToLogin: _navigateToLogin,
        isLoadingProvider: _isLoading,
        isGoogleLoadingProvider: _isGoogleLoading,
      );
    }
  }
}