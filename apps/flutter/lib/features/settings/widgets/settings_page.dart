import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/util/distance_formatter.dart';
import 'package:turbo/core/widgets/app_grouped_card.dart';
import 'package:turbo/core/widgets/app_section_header.dart';
import 'package:turbo/core/location/gps_accuracy_mode.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';
import 'package:turbo/features/settings/widgets/sections/advanced_settings_page.dart';
import 'package:turbo/features/settings/widgets/sections/appearance_settings_page.dart';
import 'package:turbo/features/settings/widgets/sections/drawing_settings_page.dart';
import 'package:turbo/features/settings/widgets/sections/location_marker_settings_page.dart';
import 'package:turbo/features/settings/widgets/sections/recording_settings_page.dart';
import 'package:turbo/features/settings/widgets/sections/units_settings_page.dart';

class SettingsPage extends ConsumerWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final settingsAsync = ref.watch(settingsProvider);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.settings)),
      body: settingsAsync.when(
        data: (settings) => Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 700),
            child: ListView(
              padding: const EdgeInsets.all(AppSpacing.l),
              children: [
                const AppSectionHeader('Personalization'),
                AppGroupedCard(
                  child: Column(
                    children: [
                      _SectionTile(
                        icon: Icons.palette_outlined,
                        title: 'Appearance',
                        subtitle: _appearanceSummary(context, settings, l10n),
                        builder: (_) => const AppearanceSettingsPage(),
                      ),
                      const _TileDivider(),
                      _SectionTile(
                        icon: Icons.straighten_outlined,
                        title: 'Units',
                        subtitle: _unitsSummary(settings, l10n),
                        builder: (_) => const UnitsSettingsPage(),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: AppSpacing.xl),
                const AppSectionHeader('Map'),
                AppGroupedCard(
                  child: Column(
                    children: [
                      _SectionTile(
                        icon: Icons.my_location_outlined,
                        title: l10n.myLocation,
                        subtitle: _locationSummary(settings),
                        builder: (_) => const LocationMarkerSettingsPage(),
                      ),
                      const _TileDivider(),
                      _SectionTile(
                        icon: Icons.gesture_outlined,
                        title: l10n.drawing,
                        subtitle: _drawingSummary(settings, l10n),
                        builder: (_) => const DrawingSettingsPage(),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: AppSpacing.xl),
                const AppSectionHeader('Tracking & data'),
                AppGroupedCard(
                  child: Column(
                    children: [
                      _SectionTile(
                        icon: Icons.fiber_manual_record_outlined,
                        title: 'Recording',
                        subtitle: _recordingSummary(settings),
                        builder: (_) => const RecordingSettingsPage(),
                      ),
                      const _TileDivider(),
                      _SectionTile(
                        icon: Icons.tune,
                        title: l10n.advanced,
                        subtitle: _advancedSummary(settings),
                        builder: (_) => const AdvancedSettingsPage(),
                      ),
                    ],
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

  String _appearanceSummary(
      BuildContext context, SettingsState s, AppLocalizations l10n) {
    final theme = switch (s.themeMode) {
      ThemeMode.light => l10n.light,
      ThemeMode.dark => l10n.dark,
      ThemeMode.system => l10n.system,
    };
    final lang = s.locale.languageCode == 'nb' ? l10n.norwegian : l10n.english;
    return '$theme · $lang';
  }

  String _unitsSummary(SettingsState s, AppLocalizations l10n) {
    return s.distanceUnit == DistanceUnit.metric
        ? l10n.distanceUnitMetric
        : l10n.distanceUnitImperial;
  }

  String _locationSummary(SettingsState s) {
    final iconLabel = switch (s.locationIconType) {
      'builtin' => 'Built-in icon',
      'custom' => 'Custom image',
      _ => 'Default dot',
    };
    return '$iconLabel · ${s.locationMarkerSize.toStringAsFixed(1)}x';
  }

  String _drawingSummary(SettingsState s, AppLocalizations l10n) {
    final smooth = s.smoothLine ? l10n.smoothLine : 'Straight line';
    return '$smooth · ${s.drawSensitivity.round()}px';
  }

  String _recordingSummary(SettingsState s) {
    final gps = switch (s.gpsAccuracyMode) {
      GpsAccuracyMode.high => 'High accuracy',
      GpsAccuracyMode.balanced => 'Balanced',
      GpsAccuracyMode.batterySaver => 'Battery saver',
    };
    return gps;
  }

  String _advancedSummary(SettingsState s) {
    return '${s.maxConcurrentDownloads} downloads · ${s.markerCacheTtlSeconds}s cache';
  }
}

class _SectionTile extends StatelessWidget {
  final IconData icon;
  final String title;
  final String subtitle;
  final WidgetBuilder builder;

  const _SectionTile({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.builder,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return ListTile(
      leading: Container(
        width: 40,
        height: 40,
        decoration: BoxDecoration(
          color: colorScheme.secondaryContainer,
          borderRadius: BorderRadius.circular(AppRadius.m),
        ),
        child: Icon(icon, color: colorScheme.onSecondaryContainer, size: 22),
      ),
      title: Text(title),
      subtitle: Text(subtitle, maxLines: 1, overflow: TextOverflow.ellipsis),
      trailing: Icon(
        Icons.chevron_right,
        color: colorScheme.onSurfaceVariant,
      ),
      onTap: () => Navigator.of(context).push(
        MaterialPageRoute(builder: builder),
      ),
    );
  }
}

class _TileDivider extends StatelessWidget {
  const _TileDivider();

  @override
  Widget build(BuildContext context) {
    return const Divider(
      height: 1,
      indent: 68,
      endIndent: AppSpacing.l,
    );
  }
}
