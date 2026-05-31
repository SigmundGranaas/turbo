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
                const SizedBox(height: AppSpacing.l),
                const AppSectionHeader('GPS accuracy'),
                AppGroupedCard(
                  padding: const EdgeInsets.all(AppSpacing.m),
                  child: SegmentedButton<GpsAccuracyMode>(
                    segments: const [
                      ButtonSegment(
                          value: GpsAccuracyMode.high, label: Text('High')),
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
                ),
                const SectionBlurb(
                    'High = best track, more battery. Saver = longer battery, sparser points.'),
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
