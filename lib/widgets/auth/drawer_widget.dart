import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/widgets/auth/user_profile_screen.dart';

import '../../data/auth/auth_providers.dart';
import 'login_modal.dart';

class AppDrawer extends ConsumerWidget {
  const AppDrawer({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Check if user is authenticated
    final authState = ref.watch(authStateProvider);
    final isAuthenticated = authState.status == AuthStatus.authenticated;
    final email = authState.email;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return NavigationDrawer(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(28, 16, 16, 10),
          child: Text(
            'Turbo',
            style: textTheme.titleLarge?.copyWith(fontWeight: FontWeight.bold),
          ),
        ),

        // User account section - made smaller and clickable
        GestureDetector(
          onTap: () {
            if (isAuthenticated) {
              Navigator.pop(context);
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) => const UserProfileScreen(),
                ),
              );
            } else {
              Navigator.pop(context);
              LoginScreen.show(context);
            }
          },
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 28, vertical: 8),
            child: Row(
              children: [
                CircleAvatar(
                  radius: 20,
                  backgroundColor: colorScheme.surface,
                  child: Text(
                    isAuthenticated && email != null && email.isNotEmpty
                        ? email[0].toUpperCase()
                        : '?',
                    style: TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.bold,
                      color: colorScheme.onSurfaceVariant,
                    ),
                  ),
                ),
                const SizedBox(width: 16),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        isAuthenticated ? 'Turbo User' : 'Guest User',
                        style: textTheme.bodyLarge,
                      ),
                      Text(
                        isAuthenticated ? (email ?? '') : 'Not signed in',
                        style: textTheme.bodyMedium?.copyWith(
                          color: colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),

        const SizedBox(height: 12),

        // Original destination items
        const NavigationDrawerDestination(
          icon: Icon(Icons.map_outlined),
          selectedIcon: Icon(Icons.map),
          label: Text('Map'),
        ),
        const NavigationDrawerDestination(
          icon: Icon(Icons.settings_outlined),
          selectedIcon: Icon(Icons.settings),
          label: Text('Settings'),
        ),

        const Divider(indent: 28, endIndent: 28),

        // Conditionally show profile option if logged in
        if (isAuthenticated)
          const NavigationDrawerDestination(
            icon: Icon(Icons.account_circle_outlined),
            selectedIcon: Icon(Icons.account_circle),
            label: Text('Profile'),
          ),

        // Authentication actions moved to a separate section
        if (isAuthenticated)
        const Divider(indent: 28, endIndent: 28),

        // Conditionally show login or logout based on auth status
        if (isAuthenticated)
          const NavigationDrawerDestination(
            icon: Icon(Icons.logout),
            label: Text('Logout'),
          )
        else
          const NavigationDrawerDestination(
            icon: Icon(Icons.login),
            label: Text('Login'),
          ),

        const SizedBox(height: 12),

        // SizedBox with flexible height to push version to bottom
        SizedBox(height: MediaQuery.of(context).size.height * 0.3),

        // App version at absolute bottom
        Padding(
          padding: const EdgeInsets.fromLTRB(28, 0, 28, 20),
          child: Text(
            'Version 1.0.0',
            style: textTheme.bodySmall?.copyWith(
              color: colorScheme.onSurfaceVariant,
            ),
          ),
        ),
      ],
      onDestinationSelected: (index) {
        Navigator.pop(context);

        // Handle navigation based on index
        int baseItems = 2; // Map, Settings
        int authOffset = isAuthenticated ? 1 : 0; // Profile adds 1 if authenticated

        if (index == 0) {
          // Map item - just close drawer as it's already the main screen
          // Add any specific navigation if needed
        } else if (index == 1) {
          // Settings item
          // Add your settings navigation logic here
        } else if (isAuthenticated && index == 2) {
          // Profile item (only for authenticated users)
          Navigator.push(
            context,
            MaterialPageRoute(
              builder: (context) => const UserProfileScreen(),
            ),
          );
        } else if (index == baseItems + authOffset) {
          // Last item is login/logout
          if (isAuthenticated) {
            _showLogoutDialog(context, ref);
          } else {
            LoginScreen.show(context);
          }
        }
      },
    );
  }

  void _showLogoutDialog(BuildContext context, WidgetRef ref) {
    // Get the auth notifier before showing the dialog
    final authNotifier = ref.read(authStateProvider.notifier);

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
              // Use the previously obtained auth notifier
              authNotifier.logout();
            },
            child: const Text('Logout'),
          ),
        ],
      ),
    );
  }
}