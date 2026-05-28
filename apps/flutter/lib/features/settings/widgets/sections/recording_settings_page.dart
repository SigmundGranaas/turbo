import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/location/gps_accuracy_mode.dart';
import 'package:turbo/core/widgets/app_grouped_card.dart';
import 'package:turbo/core/widgets/app_section_header.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';

class RecordingSettingsPage extends ConsumerWidget {
  const RecordingSettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final settingsAsync = ref.watch(settingsProvider);

    return Scaffold(
      appBar: AppBar(title: const Text('Recording')),
      body: settingsAsync.when(
        data: (settings) => Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 700),
            child: ListView(
              padding: const EdgeInsets.all(AppSpacing.l),
              children: [
                const AppSectionHeader('Screen'),
                AppGroupedCard(
                  child: SwitchListTile(
                    secondary: const Icon(Icons.screen_lock_portrait_outlined),
                    title: const Text('Keep screen on while recording'),
                    subtitle: const Text(
                        'Prevents the screen from sleeping during an active recording.'),
                    value: settings.keepScreenOnWhileRecording,
                    onChanged: (v) => ref
                        .read(settingsProvider.notifier)
                        .setKeepScreenOnWhileRecording(v),
                  ),
                ),
                const SizedBox(height: AppSpacing.xl),
                const AppSectionHeader('Location'),
                AppGroupedCard(
                  padding: const EdgeInsets.symmetric(
                      horizontal: AppSpacing.l, vertical: AppSpacing.m),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          const Icon(Icons.gps_fixed),
                          const SizedBox(width: AppSpacing.s),
                          Text('GPS accuracy',
                              style:
                                  Theme.of(context).textTheme.bodyLarge),
                        ],
                      ),
                      const SizedBox(height: AppSpacing.s),
                      SegmentedButton<GpsAccuracyMode>(
                        segments: const [
                          ButtonSegment(
                              value: GpsAccuracyMode.high,
                              label: Text('High')),
                          ButtonSegment(
                              value: GpsAccuracyMode.balanced,
                              label: Text('Balanced')),
                          ButtonSegment(
                              value: GpsAccuracyMode.batterySaver,
                              label: Text('Saver')),
                        ],
                        selected: {settings.gpsAccuracyMode},
                        onSelectionChanged: (s) => ref
                            .read(settingsProvider.notifier)
                            .setGpsAccuracyMode(s.first),
                      ),
                      const SizedBox(height: AppSpacing.s),
                      Text(
                        'High = best track, more battery. Saver = longer battery, sparser points.',
                        style:
                            Theme.of(context).textTheme.bodySmall?.copyWith(
                                  color: Theme.of(context)
                                      .colorScheme
                                      .onSurfaceVariant,
                                ),
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
}
