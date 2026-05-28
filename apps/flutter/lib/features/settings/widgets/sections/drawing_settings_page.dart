import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_grouped_card.dart';
import 'package:turbo/core/widgets/app_section_header.dart';
import 'package:turbo/features/settings/data/settings_provider.dart';

class DrawingSettingsPage extends ConsumerWidget {
  const DrawingSettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final settingsAsync = ref.watch(settingsProvider);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.drawing)),
      body: settingsAsync.when(
        data: (settings) => Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 700),
            child: ListView(
              padding: const EdgeInsets.all(AppSpacing.l),
              children: [
                const AppSectionHeader('Style'),
                AppGroupedCard(
                  child: Column(
                    children: [
                      SwitchListTile(
                        secondary:
                            const Icon(Icons.insights_outlined, size: 20),
                        title: Text(l10n.smoothLine),
                        subtitle:
                            const Text('Round corners on hand-drawn paths.'),
                        value: settings.smoothLine,
                        onChanged: (v) => ref
                            .read(settingsProvider.notifier)
                            .setSmoothLine(v),
                      ),
                      const Divider(
                          height: 1,
                          indent: AppSpacing.l,
                          endIndent: AppSpacing.l),
                      SwitchListTile(
                        secondary:
                            const Icon(Icons.linear_scale_outlined, size: 20),
                        title: Text(l10n.showPoints),
                        subtitle: const Text(
                            'Render every point captured along a path.'),
                        value: settings.showIntermediatePoints,
                        onChanged: (v) => ref
                            .read(settingsProvider.notifier)
                            .setShowIntermediatePoints(v),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: AppSpacing.l),
                const AppSectionHeader('Sensitivity'),
                AppGroupedCard(
                  padding: const EdgeInsets.symmetric(
                      horizontal: AppSpacing.l, vertical: AppSpacing.s),
                  child: Row(
                    children: [
                      Icon(Icons.line_axis,
                          size: 20,
                          color: Theme.of(context).colorScheme.onSurfaceVariant),
                      const SizedBox(width: AppSpacing.m),
                      Expanded(
                        child: Slider(
                          value: settings.drawSensitivity,
                          min: 5,
                          max: 50,
                          divisions: 9,
                          label: settings.drawSensitivity.round().toString(),
                          onChanged: (v) => ref
                              .read(settingsProvider.notifier)
                              .setDrawSensitivity(v),
                        ),
                      ),
                      Text(
                        '${settings.drawSensitivity.round()} px',
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ],
                  ),
                ),
                const SectionBlurb(
                    'How much your finger must move before a new point is added. Higher = smoother, sparser tracks.'),
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
