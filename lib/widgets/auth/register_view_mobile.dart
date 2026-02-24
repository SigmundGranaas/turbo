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
import 'package:turbo/utils.dart';

class RegisterViewMobile extends ConsumerWidget {
  final GlobalKey<FormState> formKey;
  final TextEditingController emailController;
  final TextEditingController passwordController;
  final Future<void> Function() onRegister;
  final VoidCallback onNavigateToLogin;
  final NotifierProvider<LoadingNotifier, bool> isLoadingProvider;
  final NotifierProvider<LoadingNotifier, bool> isGoogleLoadingProvider;

  const RegisterViewMobile( {
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
                      isLoading: ref.watch(isLoadingProvider),
                    ),
                    const SizedBox(height: 24),
                    AuthDivider(text: l10n.or),
                    const SizedBox(height: 24),
                    GoogleSignInButton(
                      isLoading: ref.watch(isGoogleLoadingProvider),
                      onSignInStarted: () => ref.read(isGoogleLoadingProvider.notifier).set(true),
                      onSignInCompleted: () => ref.read(isGoogleLoadingProvider.notifier).set(false),
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