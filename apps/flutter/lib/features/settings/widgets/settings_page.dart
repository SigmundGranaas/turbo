import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';
import 'package:path/path.dart' as p;
import 'package:turbo/app/location_marker_tokens.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/location/gps_accuracy_mode.dart';
import 'package:turbo/core/util/distance_formatter.dart';
import 'package:turbo/core/widgets/app_grouped_card.dart';
import 'package:turbo/core/widgets/app_section_header.dart';
import 'package:turbo/core/widgets/color_circle.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';
import 'package:turbo/features/settings/widgets/location_icon_picker_sheet.dart';
import 'package:turbo/features/sharing/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';

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
        error: (_, _) => Center(child: Text(l10n.genericLoadError)),
      ),
    );
  }

  Widget _buildSettingsList(
      BuildContext context, WidgetRef ref, SettingsState settings, AppLocalizations l10n) {
    return ListView(
      padding: const EdgeInsets.all(AppSpacing.l),
      children: [
        AppSectionHeader(l10n.theme),
        _buildThemeSelector(context, ref, settings.themeMode),
        const SizedBox(height: AppSpacing.xl),
        AppSectionHeader(l10n.language),
        _buildLanguageSelector(context, ref, settings.locale),
        const SizedBox(height: AppSpacing.xl),
        AppSectionHeader(l10n.drawing),
        _buildDrawingToggles(context, ref, settings, l10n),
        const SizedBox(height: AppSpacing.m),
        _buildSensitivitySelector(context, ref, settings.drawSensitivity, l10n),
        const SizedBox(height: AppSpacing.xl),
        AppSectionHeader(l10n.myLocation),
        _buildLocationIconPicker(context, ref, settings),
        const SizedBox(height: AppSpacing.s),
        _buildLocationSizeSlider(context, ref, settings.locationMarkerSize, l10n),
        const SizedBox(height: AppSpacing.s),
        _buildHeadingArrowToggle(context, ref, settings.showHeadingArrow, l10n),
        if (settings.showHeadingArrow) ...[
          const SizedBox(height: AppSpacing.s),
          _buildColorPickerRow(
            context,
            ref,
            label: l10n.arrowColor,
            selectedHex: settings.markerArrowColorHex,
            onColorChanged: (color) {
              ref.read(settingsProvider.notifier).setMarkerArrowColor(color);
            },
          ),
        ],
        const SizedBox(height: AppSpacing.s),
        _buildColorPickerRow(
          context,
          ref,
          label: l10n.outlineColor,
          selectedHex: settings.markerOutlineColorHex,
          onColorChanged: (color) {
            ref.read(settingsProvider.notifier).setMarkerOutlineColor(color);
          },
        ),
        const SizedBox(height: AppSpacing.xl),
        if (ref.watch(sharingAvailableProvider)) ...[
          const AppSectionHeader('Sharing'),
          AppGroupedCard(
            child: Column(children: [
              ListTile(
                leading: const Icon(Icons.person_outline),
                title: const Text('Friends'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => Navigator.of(context).push(
                  MaterialPageRoute(builder: (_) => const FriendsPage()),
                ),
              ),
              const Divider(height: 1),
              ListTile(
                leading: const Icon(Icons.group_outlined),
                title: const Text('Groups'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => Navigator.of(context).push(
                  MaterialPageRoute(builder: (_) => const GroupsPage()),
                ),
              ),
            ]),
          ),
          const SizedBox(height: AppSpacing.xl),
        ],
        const AppSectionHeader('Recording'),
        _buildKeepScreenOnToggle(context, ref, settings.keepScreenOnWhileRecording),
        const SizedBox(height: AppSpacing.s),
        _buildGpsAccuracySelector(context, ref, settings.gpsAccuracyMode),
        const SizedBox(height: AppSpacing.xl),
        AppSectionHeader(l10n.advanced),
        _buildDistanceUnitSelector(context, ref, settings.distanceUnit, l10n),
        const SizedBox(height: AppSpacing.s),
        _buildIntSliderCard(
          context,
          ref,
          icon: Icons.cloud_download_outlined,
          title: l10n.maxConcurrentDownloads,
          description: l10n.maxConcurrentDownloadsDescription,
          value: settings.maxConcurrentDownloads,
          min: kMinDownloadConcurrency,
          max: kMaxDownloadConcurrency,
          suffix: '',
          onChanged: (v) => ref
              .read(settingsProvider.notifier)
              .setMaxConcurrentDownloads(v),
        ),
        const SizedBox(height: AppSpacing.s),
        _buildIntSliderCard(
          context,
          ref,
          icon: Icons.timer_outlined,
          title: l10n.markerCacheTtl,
          description: l10n.markerCacheTtlDescription,
          value: settings.markerCacheTtlSeconds,
          min: kMinMarkerCacheTtlSeconds,
          max: kMaxMarkerCacheTtlSeconds,
          suffix: 's',
          onChanged: (v) => ref
              .read(settingsProvider.notifier)
              .setMarkerCacheTtlSeconds(v),
        ),
      ],
    );
  }

  Widget _buildKeepScreenOnToggle(
      BuildContext context, WidgetRef ref, bool value) {
    return AppGroupedCard(
      child: SwitchListTile(
        secondary: const Icon(Icons.screen_lock_portrait_outlined),
        title: const Text('Keep screen on while recording'),
        subtitle: const Text(
            'Prevents the screen from sleeping during an active recording.'),
        value: value,
        onChanged: (v) =>
            ref.read(settingsProvider.notifier).setKeepScreenOnWhileRecording(v),
      ),
    );
  }

  Widget _buildGpsAccuracySelector(
      BuildContext context, WidgetRef ref, GpsAccuracyMode current) {
    return AppGroupedCard(
      padding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.l, vertical: AppSpacing.s),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.gps_fixed),
              const SizedBox(width: AppSpacing.s),
              Text('GPS accuracy', style: Theme.of(context).textTheme.bodyLarge),
            ],
          ),
          const SizedBox(height: AppSpacing.s),
          SegmentedButton<GpsAccuracyMode>(
            segments: const [
              ButtonSegment(
                  value: GpsAccuracyMode.high, label: Text('High')),
              ButtonSegment(
                  value: GpsAccuracyMode.balanced, label: Text('Balanced')),
              ButtonSegment(
                  value: GpsAccuracyMode.batterySaver, label: Text('Saver')),
            ],
            selected: {current},
            onSelectionChanged: (s) => ref
                .read(settingsProvider.notifier)
                .setGpsAccuracyMode(s.first),
          ),
          const SizedBox(height: AppSpacing.xs),
          Text(
            'High = best track, more battery. Saver = longer battery, sparser points.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ),
    );
  }

  Widget _buildDistanceUnitSelector(BuildContext context, WidgetRef ref,
      DistanceUnit currentUnit, AppLocalizations l10n) {
    return SegmentedButton<DistanceUnit>(
      segments: <ButtonSegment<DistanceUnit>>[
        ButtonSegment<DistanceUnit>(
          value: DistanceUnit.metric,
          label: Text(l10n.distanceUnitMetric),
        ),
        ButtonSegment<DistanceUnit>(
          value: DistanceUnit.imperial,
          label: Text(l10n.distanceUnitImperial),
        ),
      ],
      selected: {currentUnit},
      onSelectionChanged: (Set<DistanceUnit> newSelection) {
        ref.read(settingsProvider.notifier).setDistanceUnit(newSelection.first);
      },
    );
  }

  Widget _buildIntSliderCard(
    BuildContext context,
    WidgetRef ref, {
    required IconData icon,
    required String title,
    required String description,
    required int value,
    required int min,
    required int max,
    required String suffix,
    required ValueChanged<int> onChanged,
  }) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return AppGroupedCard(
      padding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.l, vertical: AppSpacing.s),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(icon, size: 20),
              const SizedBox(width: AppSpacing.m),
              Expanded(
                child: Text(title, style: textTheme.bodyLarge),
              ),
              Text(
                '$value$suffix',
                style: textTheme.bodySmall,
              ),
            ],
          ),
          Slider(
            value: value.toDouble(),
            min: min.toDouble(),
            max: max.toDouble(),
            divisions: max - min,
            label: '$value$suffix',
            onChanged: (v) => onChanged(v.round()),
          ),
          Padding(
            padding: const EdgeInsets.only(bottom: AppSpacing.s),
            child: Text(
              description,
              style: textTheme.bodySmall?.copyWith(
                color: colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildDrawingToggles(
      BuildContext context, WidgetRef ref, SettingsState settings, AppLocalizations l10n) {
    return AppGroupedCard(
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
          const Divider(height: 1, indent: AppSpacing.l, endIndent: AppSpacing.l),
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
    return AppGroupedCard(
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.l, vertical: AppSpacing.s),
      child: Row(
        children: [
          const Icon(Icons.line_axis, size: 20),
          const SizedBox(width: AppSpacing.m),
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
    return AppGroupedCard(
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.l),
        onTap: () => showLocationIconPickerSheet(context, ref),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: AppSpacing.l, vertical: AppSpacing.m),
          child: Row(
            children: [
              _buildCurrentIconPreview(context, settings),
              const SizedBox(width: AppSpacing.m),
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
        color: LocationMarkerTokens.defaultFill.withValues(alpha: 0.3),
      ),
      child: Center(
        child: Container(
          width: size * 0.5,
          height: size * 0.5,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: LocationMarkerTokens.defaultFill,
            border: Border.all(color: LocationMarkerTokens.defaultOutline, width: 2),
          ),
        ),
      ),
    );
  }

  Widget _buildLocationSizeSlider(
      BuildContext context, WidgetRef ref, double currentSize, AppLocalizations l10n) {
    return AppGroupedCard(
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.l, vertical: AppSpacing.s),
      child: Row(
        children: [
          const Icon(Icons.open_in_full, size: 20),
          const SizedBox(width: AppSpacing.m),
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
    );
  }

  Widget _buildHeadingArrowToggle(
      BuildContext context, WidgetRef ref, bool showHeading, AppLocalizations l10n) {
    return AppGroupedCard(
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.l, vertical: AppSpacing.xs),
      child: Row(
        children: [
          const Icon(Icons.navigation, size: 20),
          const SizedBox(width: AppSpacing.m),
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
    );
  }

  Widget _buildColorPickerRow(
    BuildContext context,
    WidgetRef ref, {
    required String label,
    required String? selectedHex,
    required ValueChanged<Color?> onColorChanged,
  }) {
    final colorScheme = Theme.of(context).colorScheme;
    final l10n = AppLocalizations.of(context);

    return AppGroupedCard(
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.l, vertical: AppSpacing.m),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: Theme.of(context).textTheme.bodyLarge),
          const SizedBox(height: AppSpacing.s),
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: [
                ColorCircle(
                  color: null,
                  isSelected: selectedHex == null,
                  onTap: () => onColorChanged(null),
                  label: l10n.defaultColor,
                  colorScheme: colorScheme,
                ),
                ...pathColorPalette.map((color) => ColorCircle(
                      color: color,
                      isSelected: selectedHex != null &&
                          selectedHex == colorToHex(color),
                      onTap: () => onColorChanged(color),
                      colorScheme: colorScheme,
                    )),
              ],
            ),
          ),
        ],
      ),
    );
  }
}