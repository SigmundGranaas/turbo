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
      appBar: AppBar(title: const Text('Units')),
      body: settingsAsync.when(
        data: (settings) => Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 700),
            child: ListView(
              padding: const EdgeInsets.all(AppSpacing.l),
              children: [
                AppSectionHeader(l10n.distanceUnit),
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
