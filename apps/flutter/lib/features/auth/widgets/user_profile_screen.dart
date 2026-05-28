import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:turbo/core/widgets/app_dialog.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/features/settings/widgets/sections/about_settings_page.dart';

import '../data/auth_providers.dart';

class UserProfileScreen extends ConsumerWidget {
  const UserProfileScreen({super.key});

  /// Address used for the "Help & Support" tile.
  static const String _supportEmail = 'support@turbo.app';

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final authState = ref.watch(authStateProvider);
    final email = authState.email;
    final colorScheme = Theme.of(context).colorScheme;
    final authNotifier = ref.read(authStateProvider.notifier);

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.profile),
        centerTitle: true,
        actions: [
          IconButton(
            icon: const Icon(Icons.logout),
            tooltip: l10n.logout,
            onPressed: () {
              _showLogoutDialog(context, authNotifier);
            },
          ),
        ],
      ),
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(24.0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  CircleAvatar(
                    radius: 40,
                    backgroundColor: colorScheme.primaryContainer,
                    child: Text(
                      email != null && email.isNotEmpty ? email[0].toUpperCase() : '?',
                      style: Theme.of(context).textTheme.displaySmall?.copyWith(
                            color: colorScheme.onPrimaryContainer,
                            fontWeight: FontWeight.bold,
                          ),
                    ),
                  ),
                  const SizedBox(width: 16),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          email ?? l10n.user,
                          style: Theme.of(context).textTheme.titleLarge?.copyWith(
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        const SizedBox(height: 4),
                        Text(
                          authState.isGoogleUser
                              ? l10n.signedInWithGoogle
                              : l10n.turboUser,
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                            color: colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 32),

              _buildOptionTile(
                context,
                icon: Icons.settings_outlined,
                title: l10n.settings,
                onTap: () => Navigator.of(context).push(
                  MaterialPageRoute(builder: (_) => const SettingsPage()),
                ),
              ),
              _buildOptionTile(
                context,
                icon: Icons.help_outline,
                title: l10n.helpAndSupport,
                onTap: () => _openSupportEmail(context),
              ),
              _buildOptionTile(
                context,
                icon: Icons.info_outline,
                title: l10n.about,
                onTap: () => Navigator.of(context).push(
                  MaterialPageRoute(builder: (_) => const AboutSettingsPage()),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildOptionTile(
      BuildContext context, {
        required IconData icon,
        required String title,
        required VoidCallback onTap,
      }) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: Icon(icon),
      title: Text(title),
      trailing: const Icon(Icons.chevron_right),
      onTap: onTap,
    );
  }

  Future<void> _openSupportEmail(BuildContext context) async {
    final messenger = ScaffoldMessenger.of(context);
    final l10n = context.l10n;
    final uri = Uri(
      scheme: 'mailto',
      path: _supportEmail,
      query: 'subject=Turbo support',
    );
    final launched = await launchUrl(uri, mode: LaunchMode.externalApplication);
    if (!launched) {
      messenger.showSnackBar(
        SnackBar(content: Text(l10n.couldNotOpenEmailApp)),
      );
    }
  }

  Future<void> _showLogoutDialog(
      BuildContext context, AuthStateNotifier authNotifier) async {
    final l10n = context.l10n;
    final confirmed = await AppDialog.confirm(
      context,
      title: l10n.logout,
      content: l10n.areYouSureYouWantToLogout,
      confirmLabel: l10n.logout,
    );
    if (confirmed) {
      authNotifier.logout();
    }
  }
}
