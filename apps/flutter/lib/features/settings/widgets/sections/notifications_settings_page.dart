import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_grouped_card.dart';
import 'package:turbo/core/widgets/app_section_header.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';

class NotificationsSettingsPage extends ConsumerWidget {
  const NotificationsSettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final settingsAsync = ref.watch(settingsProvider);
    final notifier = ref.read(settingsProvider.notifier);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.notifications)),
      body: settingsAsync.when(
        data: (settings) {
          final masterOn = settings.pushNotificationsEnabled;
          return Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 700),
              child: ListView(
                padding: const EdgeInsets.all(AppSpacing.l),
                children: [
                  AppSectionHeader(l10n.pushNotifications),
                  AppGroupedCard(
                    child: SwitchListTile(
                      secondary: const Icon(Icons.notifications_outlined),
                      title: Text(l10n.allowPushNotifications),
                      value: masterOn,
                      onChanged: (v) =>
                          notifier.setPushNotificationsEnabled(v),
                    ),
                  ),
                  const SizedBox(height: AppSpacing.l),
                  AppSectionHeader(l10n.notifications),
                  AppGroupedCard(
                    child: Column(
                      children: [
                        SwitchListTile(
                          secondary: const Icon(Icons.person_add_outlined),
                          title: Text(l10n.friendRequests),
                          value: settings.friendRequestNotifications,
                          onChanged: masterOn
                              ? (v) =>
                                  notifier.setFriendRequestNotifications(v)
                              : null,
                        ),
                        SwitchListTile(
                          secondary: const Icon(Icons.share_outlined),
                          title: Text(l10n.shareNotifications),
                          value: settings.shareNotifications,
                          onChanged: masterOn
                              ? (v) => notifier.setShareNotifications(v)
                              : null,
                        ),
                      ],
                    ),
                  ),
                  SectionBlurb(l10n.notificationsDeliveryNote),
                ],
              ),
            ),
          );
        },
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (_, _) => Center(child: Text(l10n.genericLoadError)),
      ),
    );
  }
}
