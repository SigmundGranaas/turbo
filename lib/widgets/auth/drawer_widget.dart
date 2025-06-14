import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/widgets/auth/user_profile_screen.dart';
import 'package:turbo/widgets/auth/login_screen.dart';
import '../../data/auth/auth_providers.dart';

class AppDrawer extends ConsumerWidget {
  const AppDrawer({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Note: The following lines assume your providers are set up correctly.
    // If authStateProvider, AuthStatus, LoginScreen, or UserProfileScreen are not
    // available, you'll need to replace them with your actual implementations.
    // For demonstration, placeholder values can be used.
    final authState = ref.watch(authStateProvider);
    final isAuthenticated = authState.status == AuthStatus.authenticated;
    final email = authState.email;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    final List<Map<String, dynamic>> destinations = [
      {'key': 'map', 'icon': Icons.map_outlined, 'selectedIcon': Icons.map, 'label': 'Map'},
      {'key': 'settings', 'icon': Icons.settings_outlined, 'selectedIcon': Icons.settings, 'label': 'Settings'},
      if (isAuthenticated)
        {'key': 'profile', 'icon': Icons.account_circle_outlined, 'selectedIcon': Icons.account_circle, 'label': 'Profile'},
    ];

    return Drawer(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // --- 1. TOP HEADER SECTION (Fixed Height) ---
          Padding(
            padding: const EdgeInsets.fromLTRB(28, 28, 16, 10),
            child: Text(
              'Turbo',
              style: textTheme.titleLarge?.copyWith(fontWeight: FontWeight.bold),
            ),
          ),
          GestureDetector(
            onTap: () {
              Navigator.pop(context); // Close drawer first
              if (isAuthenticated) {
                Navigator.push(
                  context,
                  MaterialPageRoute(builder: (context) => const UserProfileScreen()),
                );
              } else {
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
          const Divider(indent: 28, endIndent: 28),

          // --- 2. NAVIGATION DESTINATIONS (FLEXIBLE/EXPANDED) ---
          // Wrap the NavigationDrawer in Expanded. This resolves the layout error by
          // giving the scrollable NavigationDrawer a fixed, finite space to occupy.
          Expanded(
            child: NavigationDrawer(
              backgroundColor: Colors.transparent,
              shadowColor: Colors.transparent,
              // You can still manage selectedIndex if you have a way to track the current route
              // selectedIndex: _calculateCurrentIndex(destinations, context),
              onDestinationSelected: (selectedIndex) {
                Navigator.pop(context);

                if (selectedIndex < 0 || selectedIndex >= destinations.length) return;
                String selectedKey = destinations[selectedIndex]['key'] as String;

                switch (selectedKey) {
                  case 'map':
                  // Navigate to Map
                    break;
                  case 'settings':
                  // Navigate to Settings
                    break;
                  case 'profile':
                    Navigator.push(
                      context,
                      MaterialPageRoute(builder: (context) => const UserProfileScreen()),
                    );
                    break;
                }
              },
              children: [
                for (var dest in destinations)
                  NavigationDrawerDestination(
                    icon: Icon(dest['icon'] as IconData),
                    selectedIcon: Icon(dest['selectedIcon'] as IconData),
                    label: Text(dest['label'] as String),
                  ),
              ],
            ),
          ),

          // --- 3. BOTTOM ACTION SECTION (Fixed Height) ---
          const Divider(indent: 28, endIndent: 28),
          if (isAuthenticated)
            ListTile(
              leading: const Icon(Icons.logout),
              title: const Text('Logout'),
              contentPadding: const EdgeInsets.symmetric(horizontal: 28.0),
              onTap: () {
                Navigator.pop(context);
                _showLogoutDialog(context, ref);
              },
            )
          else
            ListTile(
              leading: const Icon(Icons.login),
              title: const Text('Login'),
              contentPadding: const EdgeInsets.symmetric(horizontal: 28.0),
              onTap: () {
                Navigator.pop(context);
                LoginScreen.show(context);
              },
            ),
          const SizedBox(height: 12),
        ],
      ),
    );
  }

  void _showLogoutDialog(BuildContext context, WidgetRef ref) {
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
              authNotifier.logout();
            },
            child: const Text('Logout'),
          ),
        ],
      ),
    );
  }
}