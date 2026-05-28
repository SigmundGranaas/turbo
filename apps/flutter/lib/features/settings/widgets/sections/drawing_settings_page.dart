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
                AppSectionHeader('Line style'),
                AppGroupedCard(
                  child: Column(
                    children: [
                      SwitchListTile(
                        title: Text(l10n.smoothLine),
                        secondary:
                            const Icon(Icons.insights_outlined, size: 20),
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
                        title: Text(l10n.showPoints),
                        secondary:
                            const Icon(Icons.linear_scale_outlined, size: 20),
                        value: settings.showIntermediatePoints,
                        onChanged: (v) => ref
                            .read(settingsProvider.notifier)
                            .setShowIntermediatePoints(v),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: AppSpacing.xl),
                const AppSectionHeader('Input'),
                AppGroupedCard(
                  padding: const EdgeInsets.symmetric(
                      horizontal: AppSpacing.l, vertical: AppSpacing.s),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          const Icon(Icons.line_axis, size: 20),
                          const SizedBox(width: AppSpacing.m),
                          Expanded(
                            child: Text(
                              'Sensitivity',
                              style: Theme.of(context).textTheme.bodyLarge,
                            ),
                          ),
                          Text(
                            '${settings.drawSensitivity.round()}px',
                            style: Theme.of(context).textTheme.bodySmall,
                          ),
                        ],
                      ),
                      Slider(
                        value: settings.drawSensitivity,
                        min: 5,
                        max: 50,
                        divisions: 9,
                        label: settings.drawSensitivity.round().toString(),
                        onChanged: (v) => ref
                            .read(settingsProvider.notifier)
                            .setDrawSensitivity(v),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(bottom: AppSpacing.s),
                        child: Text(
                          'Larger values smooth out jitter but lose fine detail.',
                          style: Theme.of(context)
                              .textTheme
                              .bodySmall
                              ?.copyWith(
                                color: Theme.of(context)
                                    .colorScheme
                                    .onSurfaceVariant,
                              ),
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
