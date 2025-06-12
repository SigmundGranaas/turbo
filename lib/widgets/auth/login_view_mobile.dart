import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/data/auth/auth_providers.dart';
import 'package:map_app/widgets/auth/auth_divider.dart';
import 'package:map_app/widgets/auth/auth_error_message.dart';
import 'package:map_app/widgets/auth/auth_footer_link.dart';
import 'package:map_app/widgets/auth/auth_text_field.dart';
import 'package:map_app/widgets/auth/google_sign_in_button.dart';
import 'package:map_app/widgets/auth/password_field.dart';
import 'package:map_app/widgets/auth/primary_button.dart';

class LoginViewMobile extends ConsumerWidget {
  final GlobalKey<FormState> formKey;
  final TextEditingController emailController;
  final TextEditingController passwordController;
  final Future<void> Function() onLogin;
  final VoidCallback onNavigateToRegister;
  final StateProvider<bool> isLoadingProvider;
  final StateProvider<bool> isGoogleLoadingProvider;

  const LoginViewMobile({
    super.key,
    required this.formKey,
    required this.emailController,
    required this.passwordController,
    required this.onLogin,
    required this.onNavigateToRegister,
    required this.isLoadingProvider,
    required this.isGoogleLoadingProvider,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final errorMessage = ref.watch(authStateProvider.select((s) => s.errorMessage));

    return Scaffold(
      appBar: AppBar(
        title: const Text('Sign in'),
        leading: IconButton(
          icon: const Icon(Icons.close),
          onPressed: () => Navigator.of(context).pop(),
        ),
      ),
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: Form(
                key: formKey,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    if (errorMessage != null) ...[
                      AuthErrorMessage(message: errorMessage),
                      const SizedBox(height: 24),
                    ],
                    AuthTextField(
                      controller: emailController,
                      label: 'Email',
                      keyboardType: TextInputType.emailAddress,
                      validator: (val) => (val == null || val.isEmpty) ? 'Please enter your email' : null,
                    ),
                    const SizedBox(height: 16),
                    PasswordField(
                      controller: passwordController,
                      label: 'Password',
                      validator: (val) => (val == null || val.isEmpty) ? 'Please enter your password' : null,
                    ),
                    Align(
                      alignment: Alignment.centerRight,
                      child: TextButton(
                        onPressed: () { /* TODO: Implement Forgot Password */ },
                        child: const Text('Forgot Password?'),
                      ),
                    ),
                    const SizedBox(height: 16),
                    PrimaryButton(
                      text: 'Sign in',
                      onPressed: onLogin,
                      isLoading: ref.watch(isLoadingProvider),
                    ),
                    const SizedBox(height: 24),
                    const AuthDivider(text: 'or'),
                    const SizedBox(height: 24),
                    GoogleSignInButton(
                      isLoading: ref.watch(isGoogleLoadingProvider),
                      onSignInStarted: () => ref.read(isGoogleLoadingProvider.notifier).state = true,
                      onSignInCompleted: () => ref.read(isGoogleLoadingProvider.notifier).state = false,
                    ),
                    const SizedBox(height: 24),
                    AuthFooterLink(
                      message: 'Don\'t have an account?',
                      linkText: 'Create account',
                      onPressed: onNavigateToRegister,
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}