import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/auth/auth_providers.dart';
import 'package:turbo/widgets/auth/auth_divider.dart';
import 'package:turbo/widgets/auth/auth_error_message.dart';
import 'package:turbo/widgets/auth/auth_footer_link.dart';
import 'package:turbo/widgets/auth/auth_text_field.dart';
import 'package:turbo/widgets/auth/google_sign_in_button.dart';
import 'package:turbo/widgets/auth/password_field.dart';
import 'package:turbo/widgets/auth/primary_button.dart';

class LoginViewDesktop extends ConsumerWidget {
  final GlobalKey<FormState> formKey;
  final TextEditingController emailController;
  final TextEditingController passwordController;
  final Future<void> Function() onLogin;
  final VoidCallback onNavigateToRegister;
  final StateProvider<bool> isLoadingProvider;
  final StateProvider<bool> isGoogleLoadingProvider;

  const LoginViewDesktop({
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
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final errorMessage = ref.watch(authStateProvider.select((s) => s.errorMessage));

    return ClipRRect(
      borderRadius: BorderRadius.circular(28),
      child: Scaffold( // Using Scaffold for consistent background and structure
        body: SingleChildScrollView(
          padding: const EdgeInsets.fromLTRB(32, 24, 32, 32),
          child: Form(
            key: formKey,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Align(
                  alignment: Alignment.topRight,
                  child: IconButton(
                    icon: const Icon(Icons.close),
                    onPressed: () => Navigator.of(context).pop(),
                    tooltip: 'Close',
                  ),
                ),
                Text('Sign in', style: textTheme.headlineMedium),
                const SizedBox(height: 8),
                Text('to continue to Turbo', style: textTheme.bodyLarge?.copyWith(color: colorScheme.onSurfaceVariant)),
                const SizedBox(height: 32),
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
                const SizedBox(height: 24),
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
                const SizedBox(height: 24),
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
                const SizedBox(height: 32),
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
    );
  }
}