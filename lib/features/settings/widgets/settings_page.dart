import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';
import 'package:path/path.dart' as p;
import 'package:turbo/features/markers/data/icon_service.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';
import 'package:turbo/features/settings/widgets/location_icon_picker_sheet.dart';
import 'package:turbo/l10n/app_localizations.dart';

class SettingsPage extends ConsumerWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final settingsAsync = ref.watch(settingsProvider);

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.settings),
      ),
      body: settingsAsync.when(
        data: (settings) => Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 700),
            child: _buildSettingsList(context, ref, settings, l10n),
          ),
        ),
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (error, stack) => Center(child: Text('Error: $error')),
      ),
    );
  }

  Widget _buildSettingsList(
      BuildContext context, WidgetRef ref, SettingsState settings, AppLocalizations l10n) {
    return ListView(
      padding: const EdgeInsets.all(16.0),
      children: [
        _buildSectionHeader(context, l10n.theme),
        _buildThemeSelector(context, ref, settings.themeMode),
        const SizedBox(height: 24),
        _buildSectionHeader(context, l10n.language),
        _buildLanguageSelector(context, ref, settings.locale),
        const SizedBox(height: 24),
        _buildSectionHeader(context, l10n.drawing),
        _buildDrawingToggles(context, ref, settings, l10n),
        const SizedBox(height: 12),
        _buildSensitivitySelector(context, ref, settings.drawSensitivity, l10n),
        const SizedBox(height: 24),
        _buildSectionHeader(context, l10n.myLocation),
        _buildLocationIconPicker(context, ref, settings),
        const SizedBox(height: 8),
        _buildLocationSizeSlider(context, ref, settings.locationMarkerSize, l10n),
        const SizedBox(height: 8),
        _buildHeadingArrowToggle(context, ref, settings.showHeadingArrow, l10n),
      ],
    );
  }

  Widget _buildDrawingToggles(
      BuildContext context, WidgetRef ref, SettingsState settings, AppLocalizations l10n) {
    return Card(
      elevation: 0,
      color: Theme.of(context).colorScheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      child: Column(
        children: [
          SwitchListTile(
            title: Text(l10n.smoothLine),
            secondary: const Icon(Icons.insights_outlined, size: 20),
            value: settings.smoothLine,
            onChanged: (value) {
              ref.read(settingsProvider.notifier).setSmoothLine(value);
            },
          ),
          const Divider(height: 1, indent: 16, endIndent: 16),
          SwitchListTile(
            title: Text(l10n.showPoints),
            secondary: const Icon(Icons.linear_scale_outlined, size: 20),
            value: settings.showIntermediatePoints,
            onChanged: (value) {
              ref.read(settingsProvider.notifier).setShowIntermediatePoints(value);
            },
          ),
        ],
      ),
    );
  }

  Widget _buildSensitivitySelector(
      BuildContext context, WidgetRef ref, double currentSensitivity, AppLocalizations l10n) {
    return Card(
      elevation: 0,
      color: Theme.of(context).colorScheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        child: Row(
          children: [
            const Icon(Icons.line_axis, size: 20),
            const SizedBox(width: 12),
            Expanded(
              child: Slider(
                value: currentSensitivity,
                min: 5,
                max: 50,
                divisions: 9,
                label: currentSensitivity.round().toString(),
                onChanged: (value) {
                  ref.read(settingsProvider.notifier).setDrawSensitivity(value);
                },
              ),
            ),
            Text(
              "${currentSensitivity.round()}px",
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
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
        ref.read(settingsProvider.notifier).setThemeMode(newSelection.first);
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
        ref.read(settingsProvider.notifier).setLocale(newSelection.first);
      },
    );
  }

  Widget _buildLocationIconPicker(
      BuildContext context, WidgetRef ref, SettingsState settings) {
    final l10n = AppLocalizations.of(context);
    return Card(
      elevation: 0,
      color: Theme.of(context).colorScheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      child: InkWell(
        borderRadius: BorderRadius.circular(16),
        onTap: () => showLocationIconPickerSheet(context, ref),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Row(
            children: [
              _buildCurrentIconPreview(context, settings),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  l10n.locationIcon,
                  style: Theme.of(context).textTheme.bodyLarge,
                ),
              ),
              Icon(
                Icons.chevron_right,
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildCurrentIconPreview(BuildContext context, SettingsState settings) {
    const double previewSize = 40;
    final colorScheme = Theme.of(context).colorScheme;

    switch (settings.locationIconType) {
      case 'builtin':
        final iconService = IconService();
        final namedIcon = iconService.getIcon(context, settings.locationIconKey);
        return Container(
          width: previewSize,
          height: previewSize,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: colorScheme.primary.withValues(alpha: 0.15),
            border: Border.all(color: colorScheme.primary, width: 2),
          ),
          child: Icon(namedIcon.icon, size: 22, color: colorScheme.primary),
        );
      case 'custom':
        if (settings.locationImagePath != null) {
          return FutureBuilder<Directory>(
            future: getApplicationDocumentsDirectory(),
            builder: (context, snapshot) {
              if (!snapshot.hasData) {
                return _buildDefaultDotPreview(previewSize);
              }
              final fullPath =
                  p.join(snapshot.data!.path, settings.locationImagePath!);
              final file = File(fullPath);
              if (!file.existsSync()) {
                return _buildDefaultDotPreview(previewSize);
              }
              return ClipOval(
                child: SizedBox(
                  width: previewSize,
                  height: previewSize,
                  child: Image.file(file, fit: BoxFit.cover),
                ),
              );
            },
          );
        }
        return _buildDefaultDotPreview(previewSize);
      default:
        return _buildDefaultDotPreview(previewSize);
    }
  }

  Widget _buildDefaultDotPreview(double size) {
    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: Colors.lightBlue.withValues(alpha: 0.3),
      ),
      child: Center(
        child: Container(
          width: size * 0.5,
          height: size * 0.5,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: Colors.lightBlue,
            border: Border.all(color: Colors.white, width: 2),
          ),
        ),
      ),
    );
  }

  Widget _buildLocationSizeSlider(
      BuildContext context, WidgetRef ref, double currentSize, AppLocalizations l10n) {
    return Card(
      elevation: 0,
      color: Theme.of(context).colorScheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        child: Row(
          children: [
            const Icon(Icons.open_in_full, size: 20),
            const SizedBox(width: 12),
            Expanded(
              child: Slider(
                value: currentSize,
                min: 0.5,
                max: 2.0,
                divisions: 6,
                label: '${currentSize.toStringAsFixed(1)}x',
                onChanged: (value) {
                  ref
                      .read(settingsProvider.notifier)
                      .setLocationMarkerSize(value);
                },
              ),
            ),
            Text(
              '${currentSize.toStringAsFixed(1)}x',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildHeadingArrowToggle(
      BuildContext context, WidgetRef ref, bool showHeading, AppLocalizations l10n) {
    return Card(
      elevation: 0,
      color: Theme.of(context).colorScheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
        child: Row(
          children: [
            const Icon(Icons.navigation, size: 20),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(l10n.showHeadingArrow,
                      style: Theme.of(context).textTheme.bodyLarge),
                  Text(l10n.headingArrowDescription,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                            color: Theme.of(context)
                                .colorScheme
                                .onSurfaceVariant,
                          )),
                ],
              ),
            ),
            Switch(
              value: showHeading,
              onChanged: (value) {
                ref
                    .read(settingsProvider.notifier)
                    .setShowHeadingArrow(value);
              },
            ),
          ],
        ),
      ),
    );
  }
}