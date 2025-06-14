import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/auth/auth_providers.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/widgets/auth/auth_divider.dart';
import 'package:turbo/widgets/auth/auth_error_message.dart';
import 'package:turbo/widgets/auth/auth_footer_link.dart';
import 'package:turbo/widgets/auth/auth_text_field.dart';
import 'package:turbo/widgets/auth/google_sign_in_button.dart';
import 'package:turbo/widgets/auth/password_field.dart';
import 'package:turbo/widgets/auth/primary_button.dart';

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
    final l10n = context.l10n;
    final errorMessage = ref.watch(authStateProvider.select((s) => s.errorMessage));

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.signIn),
        leading: IconButton(
          icon: const Icon(Icons.close),
          tooltip: l10n.closeTooltip,
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
                      label: l10n.email,
                      keyboardType: TextInputType.emailAddress,
                      validator: (val) => (val == null || val.isEmpty) ? l10n.pleaseEnterEmail : null,
                    ),
                    const SizedBox(height: 16),
                    PasswordField(
                      controller: passwordController,
                      label: l10n.password,
                      validator: (val) => (val == null || val.isEmpty) ? l10n.pleaseEnterPassword : null,
                    ),
                    Align(
                      alignment: Alignment.centerRight,
                      child: TextButton(
                        onPressed: () { /* TODO: Implement Forgot Password */ },
                        child: Text(l10n.forgotPassword),
                      ),
                    ),
                    const SizedBox(height: 16),
                    PrimaryButton(
                      text: l10n.signIn,
                      onPressed: onLogin,
                      isLoading: ref.watch(isLoadingProvider),
                    ),
                    const SizedBox(height: 24),
                    AuthDivider(text: l10n.or),
                    const SizedBox(height: 24),
                    GoogleSignInButton(
                      isLoading: ref.watch(isGoogleLoadingProvider),
                      onSignInStarted: () => ref.read(isGoogleLoadingProvider.notifier).state = true,
                      onSignInCompleted: () => ref.read(isGoogleLoadingProvider.notifier).state = false,
                    ),
                    const SizedBox(height: 24),
                    AuthFooterLink(
                      message: l10n.dontHaveAnAccount,
                      linkText: l10n.createAccount,
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