import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_snackbars.dart';
import 'package:turbo/core/widgets/app_text_field.dart';

import '../data/auth_providers.dart';
import 'auth_error_message.dart';

class EditProfileScreen extends ConsumerStatefulWidget {
  const EditProfileScreen({super.key});

  static Future<void> show(BuildContext context) {
    final isDesktop = MediaQuery.of(context).size.width > 768;

    if (isDesktop) {
      return showDialog(
        context: context,
        builder: (context) => Dialog(
          clipBehavior: Clip.antiAlias,
          child: const SizedBox(
            width: 420,
            child: EditProfileScreen(),
          ),
        ),
      );
    } else {
      return Navigator.of(context).push(
        MaterialPageRoute(
          fullscreenDialog: true,
          builder: (context) => const EditProfileScreen(),
        ),
      );
    }
  }

  @override
  ConsumerState<EditProfileScreen> createState() => _EditProfileScreenState();
}

class _EditProfileScreenState extends ConsumerState<EditProfileScreen> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _displayNameController;

  bool _isLoading = false;
  String? _errorMessage;

  @override
  void initState() {
    super.initState();
    final displayName = ref.read(authStateProvider).displayName;
    _displayNameController = TextEditingController(text: displayName ?? '');
  }

  @override
  void dispose() {
    _displayNameController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    FocusScope.of(context).unfocus();
    setState(() => _errorMessage = null);
    if (!(_formKey.currentState?.validate() ?? false)) return;

    setState(() => _isLoading = true);
    try {
      await ref
          .read(authStateProvider.notifier)
          .updateDisplayName(_displayNameController.text.trim());
      if (!mounted) return;
      AppSnackbars.success(context, context.l10n.profileUpdated);
      Navigator.of(context).pop();
    } catch (e) {
      if (!mounted) return;
      setState(() => _errorMessage = context.l10n.couldNotUpdateProfile);
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
    final email = ref.watch(authStateProvider.select((s) => s.email));

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
          AppTextField(
            controller: _displayNameController,
            label: l10n.displayName,
          ),
          const SizedBox(height: 16),
          TextFormField(
            initialValue: email ?? '',
            enabled: false,
            decoration: InputDecoration(labelText: l10n.email),
          ),
          const SizedBox(height: 24),
          AppButton.primary(
            text: l10n.save,
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
              Text(l10n.editProfile, style: textTheme.headlineMedium),
              const SizedBox(height: 32),
              form,
            ],
          ),
        ),
      );
    }

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.editProfile),
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
