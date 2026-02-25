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
import 'package:turbo/core/theme/utils.dart';

class LoginViewDesktop extends ConsumerWidget {
  final GlobalKey<FormState> formKey;
  final TextEditingController emailController;
  final TextEditingController passwordController;
  final Future<void> Function() onLogin;
  final VoidCallback onNavigateToRegister;
  final NotifierProvider<LoadingNotifier, bool> isLoadingProvider;
  final NotifierProvider<LoadingNotifier, bool> isGoogleLoadingProvider;

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
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final errorMessage =
    ref.watch(authStateProvider.select((s) => s.errorMessage));

    return SingleChildScrollView(
      child: Padding(
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
                  tooltip: l10n.closeTooltip,
                ),
              ),
              Text(l10n.signIn, style: textTheme.headlineMedium),
              const SizedBox(height: 8),
              Text(l10n.toContinueTo(l10n.appTitle),
                  style: textTheme.bodyLarge
                      ?.copyWith(color: colorScheme.onSurfaceVariant)),
              const SizedBox(height: 32),
              if (errorMessage != null) ...[
                AuthErrorMessage(message: errorMessage),
                const SizedBox(height: 24),
              ],
              AuthTextField(
                controller: emailController,
                label: l10n.email,
                keyboardType: TextInputType.emailAddress,
                validator: (val) =>
                (val == null || val.isEmpty) ? l10n.pleaseEnterEmail : null,
              ),
              const SizedBox(height: 24),
              PasswordField(
                controller: passwordController,
                label: l10n.password,
                validator: (val) => (val == null || val.isEmpty)
                    ? l10n.pleaseEnterPassword
                    : null,
              ),
              Align(
                alignment: Alignment.centerRight,
                child: TextButton(
                  onPressed: () {},
                  child: Text(l10n.forgotPassword),
                ),
              ),
              const SizedBox(height: 24),
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
                onSignInStarted: () =>
                ref.read(isGoogleLoadingProvider.notifier).set(true),
                onSignInCompleted: () =>
                ref.read(isGoogleLoadingProvider.notifier).set(false),
              ),
              const SizedBox(height: 32),
              AuthFooterLink(
                message: l10n.dontHaveAnAccount,
                linkText: l10n.createAccount,
                onPressed: onNavigateToRegister,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
