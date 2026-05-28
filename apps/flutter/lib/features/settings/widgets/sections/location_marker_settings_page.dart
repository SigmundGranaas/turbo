import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';
import 'package:path/path.dart' as p;
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/location_marker_tokens.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_grouped_card.dart';
import 'package:turbo/core/widgets/app_section_header.dart';
import 'package:turbo/core/widgets/color_circle.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/features/saved_paths/api.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';
import 'package:turbo/features/settings/widgets/location_icon_picker_sheet.dart';

class LocationMarkerSettingsPage extends ConsumerWidget {
  const LocationMarkerSettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final settingsAsync = ref.watch(settingsProvider);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.myLocation)),
      body: settingsAsync.when(
        data: (settings) => Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 700),
            child: ListView(
              padding: const EdgeInsets.all(AppSpacing.l),
              children: [
                const AppSectionHeader('Marker'),
                _IconPickerTile(settings: settings),
                const SizedBox(height: AppSpacing.s),
                _SizeSliderTile(size: settings.locationMarkerSize),
                const SizedBox(height: AppSpacing.l),
                const AppSectionHeader('Heading'),
                _HeadingArrowTile(showHeading: settings.showHeadingArrow),
                if (settings.showHeadingArrow) ...[
                  const SizedBox(height: AppSpacing.l),
                  AppSectionHeader(l10n.arrowColor),
                  _ColorPickerCard(
                    selectedHex: settings.markerArrowColorHex,
                    onColorChanged: (color) => ref
                        .read(settingsProvider.notifier)
                        .setMarkerArrowColor(color),
                  ),
                ],
                const SizedBox(height: AppSpacing.l),
                AppSectionHeader(l10n.outlineColor),
                _ColorPickerCard(
                  selectedHex: settings.markerOutlineColorHex,
                  onColorChanged: (color) => ref
                      .read(settingsProvider.notifier)
                      .setMarkerOutlineColor(color),
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

class _IconPickerTile extends ConsumerWidget {
  final SettingsState settings;
  const _IconPickerTile({required this.settings});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    return AppGroupedCard(
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.l),
        onTap: () => showLocationIconPickerSheet(context, ref),
        child: Padding(
          padding: const EdgeInsets.symmetric(
              horizontal: AppSpacing.l, vertical: AppSpacing.m),
          child: Row(
            children: [
              _IconPreview(settings: settings),
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
}

class _IconPreview extends StatelessWidget {
  final SettingsState settings;
  const _IconPreview({required this.settings});

  @override
  Widget build(BuildContext context) {
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
                return _defaultDot(previewSize);
              }
              final fullPath =
                  p.join(snapshot.data!.path, settings.locationImagePath!);
              final file = File(fullPath);
              if (!file.existsSync()) {
                return _defaultDot(previewSize);
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
        return _defaultDot(previewSize);
      default:
        return _defaultDot(previewSize);
    }
  }

  Widget _defaultDot(double size) {
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
            border: Border.all(
                color: LocationMarkerTokens.defaultOutline, width: 2),
          ),
        ),
      ),
    );
  }
}

class _SizeSliderTile extends ConsumerWidget {
  final double size;
  const _SizeSliderTile({required this.size});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return AppGroupedCard(
      padding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.l, vertical: AppSpacing.s),
      child: Row(
        children: [
          const Icon(Icons.open_in_full, size: 20),
          const SizedBox(width: AppSpacing.m),
          Expanded(
            child: Slider(
              value: size,
              min: 0.5,
              max: 2.0,
              divisions: 6,
              label: '${size.toStringAsFixed(1)}x',
              onChanged: (v) => ref
                  .read(settingsProvider.notifier)
                  .setLocationMarkerSize(v),
            ),
          ),
          Text(
            '${size.toStringAsFixed(1)}x',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ),
    );
  }
}

class _HeadingArrowTile extends ConsumerWidget {
  final bool showHeading;
  const _HeadingArrowTile({required this.showHeading});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    return AppGroupedCard(
      padding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.l, vertical: AppSpacing.xs),
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
            onChanged: (v) => ref
                .read(settingsProvider.notifier)
                .setShowHeadingArrow(v),
          ),
        ],
      ),
    );
  }
}

class _ColorPickerCard extends StatelessWidget {
  final String? selectedHex;
  final ValueChanged<Color?> onColorChanged;

  const _ColorPickerCard({
    required this.selectedHex,
    required this.onColorChanged,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final l10n = AppLocalizations.of(context);

    return AppGroupedCard(
      padding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.l, vertical: AppSpacing.m),
      child: SingleChildScrollView(
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
    );
  }
}
