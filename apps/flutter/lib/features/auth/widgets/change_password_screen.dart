import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';

import '../data/auth_providers.dart';
import 'auth_error_message.dart';
import 'password_field.dart';

/// Minimum password length, matching the register flow validators.
const int _kMinPasswordLength = 8;

class ChangePasswordScreen extends ConsumerStatefulWidget {
  const ChangePasswordScreen({super.key});

  static Future<void> show(BuildContext context) {
    final isDesktop = MediaQuery.of(context).size.width > 768;

    if (isDesktop) {
      return showDialog(
        context: context,
        builder: (context) => Dialog(
          clipBehavior: Clip.antiAlias,
          child: const SizedBox(
            width: 420,
            child: ChangePasswordScreen(),
          ),
        ),
      );
    } else {
      return Navigator.of(context).push(
        MaterialPageRoute(
          fullscreenDialog: true,
          builder: (context) => const ChangePasswordScreen(),
        ),
      );
    }
  }

  @override
  ConsumerState<ChangePasswordScreen> createState() =>
      _ChangePasswordScreenState();
}

class _ChangePasswordScreenState extends ConsumerState<ChangePasswordScreen> {
  final _formKey = GlobalKey<FormState>();
  final _currentController = TextEditingController();
  final _newController = TextEditingController();
  final _confirmController = TextEditingController();

  bool _isLoading = false;
  String? _errorMessage;

  @override
  void dispose() {
    _currentController.dispose();
    _newController.dispose();
    _confirmController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    FocusScope.of(context).unfocus();
    setState(() => _errorMessage = null);
    if (!(_formKey.currentState?.validate() ?? false)) return;

    setState(() => _isLoading = true);
    try {
      await ref.read(authStateProvider.notifier).changePassword(
            _currentController.text,
            _newController.text,
            _confirmController.text,
          );
      if (!mounted) return;
      AppSnackbars.success(context, context.l10n.passwordChanged);
      Navigator.of(context).pop();
    } catch (e) {
      if (!mounted) return;
      setState(() => _errorMessage = context.l10n.couldNotChangePassword);
    } finally {
      if (mounted) {
        setState(() => _isLoading = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final isDesktop = MediaQuery.of(context).size.width > 768;
    final textTheme = Theme.of(context).textTheme;

    final form = Form(
      key: _formKey,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (_errorMessage != null) ...[
            AuthErrorMessage(message: _errorMessage!),
            const SizedBox(height: 24),
          ],
          PasswordField(
            controller: _currentController,
            label: l10n.currentPassword,
            validator: (val) =>
                (val == null || val.isEmpty) ? l10n.pleaseEnterPassword : null,
          ),
          const SizedBox(height: 16),
          PasswordField(
            controller: _newController,
            label: l10n.newPassword,
            validator: (val) {
              if (val == null || val.isEmpty) return l10n.pleaseEnterPassword;
              if (val.length < _kMinPasswordLength) {
                return l10n.passwordTooShort(_kMinPasswordLength);
              }
              return null;
            },
          ),
          const SizedBox(height: 16),
          PasswordField(
            controller: _confirmController,
            label: l10n.confirmNewPassword,
            validator: (val) {
              if (val == null || val.isEmpty) return l10n.pleaseEnterPassword;
              if (val != _newController.text) return l10n.passwordsDoNotMatch;
              return null;
            },
          ),
          const SizedBox(height: 24),
          AppButton.primary(
            text: l10n.changePassword,
            onPressed: _submit,
            isLoading: _isLoading,
            fullWidth: true,
          ),
        ],
      ),
    );

    if (isDesktop) {
      return SingleChildScrollView(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(32, 24, 32, 32),
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
              Text(l10n.changePassword, style: textTheme.headlineMedium),
              const SizedBox(height: 32),
              form,
            ],
          ),
        ),
      );
    }

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.changePassword),
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
              child: form,
            ),
          ),
        ),
      ),
    );
  }
}
