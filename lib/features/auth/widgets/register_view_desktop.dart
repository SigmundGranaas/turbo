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

class RegisterViewDesktop extends ConsumerWidget {
  final GlobalKey<FormState> formKey;
  final TextEditingController emailController;
  final TextEditingController passwordController;
  final Future<void> Function() onRegister;
  final VoidCallback onNavigateToLogin;
  final bool isLoading;
  final bool isGoogleLoading;
  final VoidCallback onGoogleSignInStarted;
  final VoidCallback onGoogleSignInCompleted;

  const RegisterViewDesktop({
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
              Text(l10n.createAccount, style: textTheme.headlineMedium),
              const SizedBox(height: 32),
              if (errorMessage != null) ...[
                AuthErrorMessage(message: errorMessage),
                const SizedBox(height: 24),
              ],
              AppTextField(
                controller: emailController,
                label: l10n.email,
                keyboardType: TextInputType.emailAddress,
                validator: (val) {
                  if (val == null || val.isEmpty) return l10n.pleaseEnterEmail;
                  if ((!RegExp(r'^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$').hasMatch(val)))
                  {
                    return l10n.pleaseEnterValidEmail;
                  }

                  return null;
                },
              ),
              const SizedBox(height: 24),
              PasswordField(
                controller: passwordController,
                label: l10n.password,
                validator: (val) {
                  if (val == null || val.isEmpty) {
                    return l10n.pleaseEnterPassword;
                  }
                  if (val.length < 8){
                    return l10n.passwordTooShort(8);
                  }
                  return null;
                },
              ),
              const SizedBox(height: 32),
              AppButton.primary(
                text: l10n.createAccount,
                onPressed: onRegister,
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
              Text(
                l10n.termsAndPrivacy,
                textAlign: TextAlign.center,
                style: textTheme.bodySmall
                    ?.copyWith(color: colorScheme.onSurfaceVariant),
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
    );
  }
}
