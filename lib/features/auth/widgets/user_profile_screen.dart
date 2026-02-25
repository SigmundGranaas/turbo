import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/l10n/app_localizations.dart';

import '../data/auth_providers.dart';

class UserProfileScreen extends ConsumerWidget {
  const UserProfileScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final email = ref.watch(authStateProvider).email;
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
                      style: TextStyle(
                        fontSize: 36,
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
                          l10n.turboUser,
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
                icon: Icons.person_outline,
                title: l10n.editProfile,
                onTap: () {},
              ),
              _buildOptionTile(
                context,
                icon: Icons.lock_outline,
                title: l10n.changePassword,
                onTap: () {},
              ),
              _buildOptionTile(
                context,
                icon: Icons.notifications_outlined,
                title: l10n.notifications,
                onTap: () {},
              ),
              _buildOptionTile(
                context,
                icon: Icons.help_outline,
                title: l10n.helpAndSupport,
                onTap: () {},
              ),
              _buildOptionTile(
                context,
                icon: Icons.info_outline,
                title: l10n.about,
                onTap: () {},
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

  void _showLogoutDialog(BuildContext context, AuthStateNotifier authNotifier) {
    final l10n = context.l10n;
    showDialog(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: Text(l10n.logout),
        content: Text(l10n.areYouSureYouWantToLogout),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            onPressed: () {
              Navigator.of(dialogContext).pop();
              authNotifier.logout();
            },
            child: Text(l10n.logout),
          ),
        ],
      ),
    );
  }
}