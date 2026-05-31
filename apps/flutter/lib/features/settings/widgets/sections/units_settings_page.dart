import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/util/distance_formatter.dart';
import 'package:turbo/core/widgets/app_grouped_card.dart';
import 'package:turbo/core/widgets/app_section_header.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';

class UnitsSettingsPage extends ConsumerWidget {
  const UnitsSettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final settingsAsync = ref.watch(settingsProvider);

    return Scaffold(
      appBar: AppBar(title: const Text('Units & downloads')),
      body: settingsAsync.when(
        data: (settings) => Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 700),
            child: ListView(
              padding: const EdgeInsets.all(AppSpacing.l),
              children: [
                const AppSectionHeader('Distance'),
                AppGroupedCard(
                  padding: const EdgeInsets.all(AppSpacing.m),
                  child: SegmentedButton<DistanceUnit>(
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
                    selected: {settings.distanceUnit},
                    onSelectionChanged: (s) => ref
                        .read(settingsProvider.notifier)
                        .setDistanceUnit(s.first),
                  ),
                ),
                const SectionBlurb(
                    'Metric uses km and m. Imperial uses miles and ft.'),
                const SizedBox(height: AppSpacing.s),
                const AppSectionHeader('Downloads'),
                _IntSliderCard(
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
                _IntSliderCard(
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
            ),
          ),
        ),
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (_, _) => Center(child: Text(l10n.genericLoadError)),
      ),
    );
  }
}

class _IntSliderCard extends StatelessWidget {
  final IconData icon;
  final String title;
  final String description;
  final int value;
  final int min;
  final int max;
  final String suffix;
  final ValueChanged<int> onChanged;

  const _IntSliderCard({
    required this.icon,
    required this.title,
    required this.description,
    required this.value,
    required this.min,
    required this.max,
    required this.suffix,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
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
              Icon(icon, size: 20, color: colorScheme.onSurfaceVariant),
              const SizedBox(width: AppSpacing.m),
              Expanded(
                child: Text(title, style: textTheme.bodyLarge),
              ),
              Text('$value$suffix',
                  style: textTheme.bodySmall
                      ?.copyWith(color: colorScheme.onSurfaceVariant)),
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
              style: textTheme.bodySmall
                  ?.copyWith(color: colorScheme.onSurfaceVariant),
            ),
          ),
        ],
      ),
    );
  }
}
