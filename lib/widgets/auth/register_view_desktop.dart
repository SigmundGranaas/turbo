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

class RegisterViewDesktop extends ConsumerWidget {
  final GlobalKey<FormState> formKey;
  final TextEditingController emailController;
  final TextEditingController passwordController;
  final Future<void> Function() onRegister;
  final VoidCallback onNavigateToLogin;
  final StateProvider<bool> isLoadingProvider;
  final StateProvider<bool> isGoogleLoadingProvider;

  const RegisterViewDesktop({
    super.key,
    required this.formKey,
    required this.emailController,
    required this.passwordController,
    required this.onRegister,
    required this.onNavigateToLogin,
    required this.isLoadingProvider,
    required this.isGoogleLoadingProvider,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final errorMessage = ref.watch(authStateProvider.select((s) => s.errorMessage));

    return ClipRRect(
      borderRadius: BorderRadius.circular(28),
      child: Scaffold(
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
                Text('Create account', style: textTheme.headlineMedium),
                const SizedBox(height: 8),
                const SizedBox(height: 32),
                if (errorMessage != null) ...[
                  AuthErrorMessage(message: errorMessage),
                  const SizedBox(height: 24),
                ],
                AuthTextField(
                  controller: emailController,
                  label: 'Email',
                  keyboardType: TextInputType.emailAddress,
                  validator: (val) {
                    if (val == null || val.isEmpty) return 'Please enter an email';
                    if (!RegExp(r'^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$').hasMatch(val)) return 'Please enter a valid email';
                    return null;
                  },
                ),
                const SizedBox(height: 24),
                PasswordField(
                  controller: passwordController,
                  label: 'Password',
                  validator: (val) {
                    if (val == null || val.isEmpty) return 'Please enter a password';
                    if (val.length < 8) return 'Password must be at least 8 characters';
                    return null;
                  },
                ),
                const SizedBox(height: 32),
                PrimaryButton(
                  text: 'Create account',
                  onPressed: onRegister,
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
                Text(
                  'By creating an account, you agree to our Terms of Service and Privacy Policy.',
                  textAlign: TextAlign.center,
                  style: textTheme.bodySmall?.copyWith(color: colorScheme.onSurfaceVariant),
                ),
                const SizedBox(height: 16),
                AuthFooterLink(
                  message: 'Already have an account?',
                  linkText: 'Sign in',
                  onPressed: onNavigateToLogin,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}