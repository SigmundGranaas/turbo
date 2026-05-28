import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_grouped_card.dart';
import 'package:turbo/core/widgets/app_section_header.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';

class AppearanceSettingsPage extends ConsumerWidget {
  const AppearanceSettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final settingsAsync = ref.watch(settingsProvider);

    return Scaffold(
      appBar: AppBar(title: const Text('Appearance')),
      body: settingsAsync.when(
        data: (settings) => Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 700),
            child: ListView(
              padding: const EdgeInsets.all(AppSpacing.l),
              children: [
                AppSectionHeader(l10n.theme),
                AppGroupedCard(
                  padding: const EdgeInsets.all(AppSpacing.m),
                  child: SegmentedButton<ThemeMode>(
                    segments: <ButtonSegment<ThemeMode>>[
                      ButtonSegment<ThemeMode>(
                        value: ThemeMode.light,
                        label: Text(l10n.light),
                        icon: const Icon(Icons.light_mode_outlined),
                      ),
                      ButtonSegment<ThemeMode>(
                        value: ThemeMode.dark,
                        label: Text(l10n.dark),
                        icon: const Icon(Icons.dark_mode_outlined),
                      ),
                      ButtonSegment<ThemeMode>(
                        value: ThemeMode.system,
                        label: Text(l10n.system),
                        icon: const Icon(Icons.brightness_auto_outlined),
                      ),
                    ],
                    selected: {settings.themeMode},
                    onSelectionChanged: (s) => ref
                        .read(settingsProvider.notifier)
                        .setThemeMode(s.first),
                  ),
                ),
                const SizedBox(height: AppSpacing.xl),
                AppSectionHeader(l10n.language),
                AppGroupedCard(
                  padding: const EdgeInsets.all(AppSpacing.m),
                  child: SegmentedButton<Locale>(
                    segments: <ButtonSegment<Locale>>[
                      ButtonSegment<Locale>(
                        value: const Locale('en'),
                        label: Text(l10n.english),
                      ),
                      ButtonSegment<Locale>(
                        value: const Locale('nb'),
                        label: Text(l10n.norwegian),
                      ),
                    ],
                    selected: {settings.locale},
                    onSelectionChanged: (s) => ref
                        .read(settingsProvider.notifier)
                        .setLocale(s.first),
                  ),
                ),
              ],
            ),
          ),
        ),
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (_, _) => Center(child: Text(l10n.genericLoadError)),
      ),
    );
  }
}
