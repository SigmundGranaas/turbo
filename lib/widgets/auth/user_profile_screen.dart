import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/auth/auth_providers.dart';

class UserProfileScreen extends ConsumerWidget {
  const UserProfileScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final email = ref.watch(authStateProvider).email;
    final colorScheme = Theme.of(context).colorScheme;
    // Get the auth notifier at build time
    final authNotifier = ref.read(authStateProvider.notifier);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Profile'),
        centerTitle: true,
        actions: [
          IconButton(
            icon: const Icon(Icons.logout),
            onPressed: () {
              // Pass the already obtained authNotifier to the dialog
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
              // Profile header
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
                          email ?? 'User',
                          style: Theme.of(context).textTheme.titleLarge?.copyWith(
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        const SizedBox(height: 4),
                        Text(
                          'Turbo User',
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

              // Profile options
              _buildOptionTile(
                context,
                icon: Icons.person_outline,
                title: 'Edit Profile',
                onTap: () {
                  // Navigate to edit profile page
                },
              ),
              _buildOptionTile(
                context,
                icon: Icons.lock_outline,
                title: 'Change Password',
                onTap: () {
                  // Navigate to change password page
                },
              ),
              _buildOptionTile(
                context,
                icon: Icons.notifications_outlined,
                title: 'Notifications',
                onTap: () {
                  // Navigate to notifications page
                },
              ),
              _buildOptionTile(
                context,
                icon: Icons.help_outline,
                title: 'Help & Support',
                onTap: () {
                  // Navigate to help page
                },
              ),
              _buildOptionTile(
                context,
                icon: Icons.info_outline,
                title: 'About',
                onTap: () {
                  // Navigate to about page
                },
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
    showDialog(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: const Text('Logout'),
        content: const Text('Are you sure you want to logout?'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () {
              Navigator.of(dialogContext).pop();
              // Use the authNotifier passed from build method
              authNotifier.logout();
            },
            child: const Text('Logout'),
          ),
        ],
      ),
    );
  }
}