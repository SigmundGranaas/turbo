import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../data/auth_providers.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import './auth_divider.dart';
import './auth_error_message.dart';
import './auth_footer_link.dart';
import './google_sign_in_button.dart';
import './password_field.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_text_field.dart';

class LoginViewMobile extends ConsumerWidget {
  final GlobalKey<FormState> formKey;
  final TextEditingController emailController;
  final TextEditingController passwordController;
  final Future<void> Function() onLogin;
  final VoidCallback onNavigateToRegister;
  final bool isLoading;
  final bool isGoogleLoading;
  final VoidCallback onGoogleSignInStarted;
  final VoidCallback onGoogleSignInCompleted;

  const LoginViewMobile({
    super.key,
    required this.formKey,
    required this.emailController,
    required this.passwordController,
    required this.onLogin,
    required this.onNavigateToRegister,
    required this.isLoading,
    required this.isGoogleLoading,
    required this.onGoogleSignInStarted,
    required this.onGoogleSignInCompleted,
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
                    AppTextField(
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
                    AppButton.primary(
                      text: l10n.signIn,
                      onPressed: onLogin,
                      isLoading: isLoading,
                      fullWidth: true,
                    ),
                    const SizedBox(height: 24),
                    AuthDivider(text: l10n.or),
                    const SizedBox(height: 24),
                    GoogleSignInButton(
                      isLoading: isGoogleLoading,
                      onSignInStarted: onGoogleSignInStarted,
                      onSignInCompleted: onGoogleSignInCompleted,
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
