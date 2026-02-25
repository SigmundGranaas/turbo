import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/settings/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/widgets/offline_regions_page.dart';
import 'package:turbo/l10n/app_localizations.dart';
import './user_profile_screen.dart';
import './login_screen.dart';
import '../data/auth_providers.dart';

class AppDrawer extends ConsumerWidget {
  const AppDrawer({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final authState = ref.watch(authStateProvider);
    final isAuthenticated = authState.status == AuthStatus.authenticated;
    final email = authState.email;
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    final List<Map<String, dynamic>> destinations = [
      {'key': 'map', 'icon': Icons.map_outlined, 'selectedIcon': Icons.map, 'label': l10n.map},
      {'key': 'offline_maps', 'icon': Icons.download_for_offline_outlined, 'selectedIcon': Icons.download_for_offline, 'label': "Offline Maps"},
      {'key': 'settings', 'icon': Icons.settings_outlined, 'selectedIcon': Icons.settings, 'label': l10n.settings},
      if (isAuthenticated)
        {'key': 'profile', 'icon': Icons.account_circle_outlined, 'selectedIcon': Icons.account_circle, 'label': l10n.profile},
    ];

    return Drawer(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SafeArea(
            bottom: false,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(28, 20, 16, 10),
                  child: Text(
                    l10n.appTitle,
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
                          backgroundColor: colorScheme.surfaceContainer,
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
                                isAuthenticated ? l10n.turboUser : l10n.guestUser,
                                style: textTheme.bodyLarge,
                              ),
                              Text(
                                isAuthenticated ? (email ?? '') : l10n.notSignedIn,
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
              ],
            ),
          ),
          const SizedBox(height: 12),
          const Divider(indent: 28, endIndent: 28),
          Expanded(
            child: NavigationDrawer(
              backgroundColor: Colors.transparent,
              shadowColor: Colors.transparent,
              onDestinationSelected: (selectedIndex) {
                Navigator.pop(context);

                if (selectedIndex < 0 || selectedIndex >= destinations.length) return;
                String selectedKey = destinations[selectedIndex]['key'] as String;

                switch (selectedKey) {
                  case 'map':
                  // Already on map, do nothing
                    break;
                  case 'offline_maps':
                    Navigator.push(
                      context,
                      MaterialPageRoute(builder: (context) => const OfflineRegionsPage()),
                    );
                    break;
                  case 'settings':
                    Navigator.push(
                      context,
                      MaterialPageRoute(builder: (context) => const SettingsPage()),
                    );
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
          SafeArea(
            top: false,
            child: Column(
              children: [
                const Divider(indent: 28, endIndent: 28),
                if (isAuthenticated)
                  ListTile(
                    leading: const Icon(Icons.logout),
                    title: Text(l10n.logout),
                    contentPadding: const EdgeInsets.symmetric(horizontal: 28.0),
                    onTap: () {
                      Navigator.pop(context);
                      _showLogoutDialog(context, ref);
                    },
                  )
                else
                  ListTile(
                    leading: const Icon(Icons.login),
                    title: Text(l10n.login),
                    contentPadding: const EdgeInsets.symmetric(horizontal: 28.0),
                    onTap: () {
                      Navigator.pop(context);
                      LoginScreen.show(context);
                    },
                  ),
                const SizedBox(height: 12),
              ],
            ),
          ),
        ],
      ),
    );
  }

  void _showLogoutDialog(BuildContext context, WidgetRef ref) {
    final authNotifier = ref.read(authStateProvider.notifier);
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
