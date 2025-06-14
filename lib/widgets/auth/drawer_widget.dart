import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/widgets/auth/user_profile_screen.dart';
import 'package:turbo/widgets/auth/login_screen.dart';
import '../../data/auth/auth_providers.dart';

class AppDrawer extends ConsumerWidget {
  const AppDrawer({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final authState = ref.watch(authStateProvider);
    final isAuthenticated = authState.status == AuthStatus.authenticated;
    final email = authState.email;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Drawer(
      child: Column(
        children: [
          _buildHeader(context, isAuthenticated, email, colorScheme, textTheme),
          ListTile(
            leading: const Icon(Icons.map_outlined),
            title: const Text('Map'),
            onTap: () => Navigator.pop(context),
          ),
          ListTile(
            leading: const Icon(Icons.settings_outlined),
            title: const Text('Settings'),
            onTap: () {
              Navigator.pop(context);
            },
          ),
          if (isAuthenticated)
            ListTile(
              leading: const Icon(Icons.account_circle_outlined),
              title: const Text('Profile'),
              onTap: () {
                Navigator.pop(context);
                Navigator.push(
                  context,
                  MaterialPageRoute(builder: (context) => const UserProfileScreen()),
                );
              },
            ),
          const Spacer(),
          if (isAuthenticated)
            ListTile(
              leading: const Icon(Icons.logout),
              title: const Text('Logout'),
              onTap: () => _showLogoutDialog(context, ref),
            )
          else
            ListTile(
              leading: const Icon(Icons.login),
              title: const Text('Login / Register'),
              onTap: () {
                Navigator.pop(context);
                LoginScreen.show(context);
              },
            ),
        ],
      ),
    );
  }

  Widget _buildHeader(BuildContext context, bool isAuthenticated, String? email,
      ColorScheme colorScheme, TextTheme textTheme) {
    return UserAccountsDrawerHeader(
      accountName: Text(
        isAuthenticated ? 'Turbo User' : 'Guest',
        style: textTheme.titleMedium?.copyWith(
          color: colorScheme.onSurface,
          fontWeight: FontWeight.bold,
        ),
      ),
      accountEmail: Text(
        isAuthenticated ? (email ?? '') : 'Not signed in',
        style: textTheme.bodyMedium?.copyWith(
          color: colorScheme.onSurfaceVariant,
        ),
      ),
      currentAccountPicture: CircleAvatar(
        backgroundColor: colorScheme.primaryContainer,
        child: Text(
          isAuthenticated && email != null && email.isNotEmpty
              ? email[0].toUpperCase()
              : 'G',
          style: textTheme.titleLarge?.copyWith(
            color: colorScheme.onPrimaryContainer,
            fontWeight: FontWeight.bold,
          ),
        ),
      ),
      decoration: BoxDecoration(
        color: colorScheme.surfaceContainerLow,
      ),
      onDetailsPressed: () {
        if (isAuthenticated) {
          Navigator.pop(context);
          Navigator.push(
            context,
            MaterialPageRoute(
              builder: (context) => const UserProfileScreen(),
            ),
          );
        }
      },
    );
  }

  void _showLogoutDialog(BuildContext context, WidgetRef ref) {
    final authNotifier = ref.read(authStateProvider.notifier);

    showDialog(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog.adaptive(
        title: const Text('Logout'),
        content: const Text('Are you sure you want to log out?'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () {
              Navigator.of(dialogContext).pop();
              Navigator.of(context).pop();
              authNotifier.logout();
            },
            child: const Text('Logout'),
          ),
        ],
      ),
    );
  }
}