import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../data/auth_providers.dart';
import 'package:turbo/l10n/app_localizations.dart';
import './auth_divider.dart';
import './auth_error_message.dart';
import './auth_footer_link.dart';
import './auth_text_field.dart';
import './google_sign_in_button.dart';
import './password_field.dart';
import 'package:turbo/core/widgets/buttons/primary_button.dart';

class RegisterViewMobile extends ConsumerWidget {
  final GlobalKey<FormState> formKey;
  final TextEditingController emailController;
  final TextEditingController passwordController;
  final Future<void> Function() onRegister;
  final VoidCallback onNavigateToLogin;
  final bool isLoading;
  final bool isGoogleLoading;
  final VoidCallback onGoogleSignInStarted;
  final VoidCallback onGoogleSignInCompleted;

  const RegisterViewMobile({
    super.key,
    required this.formKey,
    required this.emailController,
    required this.passwordController,
    required this.onRegister,
    required this.onNavigateToLogin,
    required this.isLoading,
    required this.isGoogleLoading,
    required this.onGoogleSignInStarted,
    required this.onGoogleSignInCompleted,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final errorMessage = ref.watch(authStateProvider.select((s) => s.errorMessage));

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.createAccount),
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
                      validator: (val) {
                        if (val == null || val.isEmpty) return l10n.pleaseEnterEmail;
                        if (!RegExp(r'^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$').hasMatch(val)) return l10n.pleaseEnterValidEmail;
                        return null;
                      },
                    ),
                    const SizedBox(height: 16),
                    PasswordField(
                      controller: passwordController,
                      label: l10n.password,
                      validator: (val) {
                        if (val == null || val.isEmpty) return l10n.pleaseEnterPassword;
                        if (val.length < 8) return l10n.passwordTooShort(8);
                        return null;
                      },
                    ),
                    const SizedBox(height: 32),
                    PrimaryButton(
                      text: l10n.createAccount,
                      onPressed: onRegister,
                      isLoading: isLoading,
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
                    Text(
                      l10n.termsAndPrivacy,
                      textAlign: TextAlign.center,
                      style: textTheme.bodySmall?.copyWith(color: colorScheme.onSurfaceVariant),
                    ),
                    const SizedBox(height: 16),
                    AuthFooterLink(
                      message: l10n.alreadyHaveAnAccount,
                      linkText: l10n.signIn,
                      onPressed: onNavigateToLogin,
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
