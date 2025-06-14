import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../data/state/providers/settings_provider.dart';
import '../../l10n/app_localizations.dart';

class SettingsPage extends ConsumerWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final themeMode = ref.watch(themeModeProvider);
    final locale = ref.watch(localeProvider);

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.settings),
      ),
      body: ListView(
        padding: const EdgeInsets.all(16.0),
        children: [
          _buildSectionHeader(context, l10n.theme),
          _buildThemeSelector(context, ref, themeMode),
          const SizedBox(height: 24),
          _buildSectionHeader(context, l10n.language),
          _buildLanguageSelector(context, ref, locale),
        ],
      ),
    );
  }

  Widget _buildSectionHeader(BuildContext context, String title) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8.0),
      child: Text(
        title,
        style: Theme.of(context).textTheme.titleMedium?.copyWith(
          color: Theme.of(context).colorScheme.primary,
          fontWeight: FontWeight.bold,
        ),
      ),
    );
  }

  Widget _buildThemeSelector(BuildContext context, WidgetRef ref, ThemeMode currentMode) {
    final l10n = AppLocalizations.of(context);
    return SegmentedButton<ThemeMode>(
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
      selected: {currentMode},
      onSelectionChanged: (Set<ThemeMode> newSelection) {
        ref.read(themeModeProvider.notifier).setThemeMode(newSelection.first);
      },
    );
  }

  Widget _buildLanguageSelector(BuildContext context, WidgetRef ref, Locale currentLocale) {
    final l10n = AppLocalizations.of(context);
    return SegmentedButton<Locale>(
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
      selected: {currentLocale},
      onSelectionChanged: (Set<Locale> newSelection) {
        ref.read(localeProvider.notifier).setLocale(newSelection.first);
      },
    );
  }
}