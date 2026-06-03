import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/sheet_drag_handle.dart';
import '../data/avalanche_forecast_notifier.dart';
import '../models/avalanche_warning.dart';

class AvalancheWarningSheet extends ConsumerWidget {
  final LatLng position;
  const AvalancheWarningSheet({super.key, required this.position});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(avalancheForecastProvider(position));
    final scheme = Theme.of(context).colorScheme;

    return DraggableScrollableSheet(
      initialChildSize: 0.55,
      minChildSize: 0.3,
      maxChildSize: 0.9,
      expand: false,
      builder: (context, controller) {
        return Container(
          decoration: BoxDecoration(
            color: scheme.surface,
            borderRadius:
                const BorderRadius.vertical(top: Radius.circular(24)),
          ),
          child: async.when(
            loading: () => const Center(child: CircularProgressIndicator()),
            error: (_, _) => Center(
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: Text(context.l10n.avalancheLoadError),
              ),
            ),
            data: (w) =>
                w == null ? _noData(context) : _body(context, controller, w),
          ),
        );
      },
    );
  }

  Widget _noData(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Text(context.l10n.avalancheLoadError),
      ),
    );
  }

  Widget _body(BuildContext context, ScrollController controller,
      AvalancheWarning w) {
    final l10n = context.l10n;
    final theme = Theme.of(context);
    return ListView(
      controller: controller,
      padding: const EdgeInsets.fromLTRB(20, 16, 20, 24),
      children: [
        const SheetDragHandle(),
        const SizedBox(height: 16),
        Text(l10n.avalancheForecast, style: theme.textTheme.titleLarge),
        const SizedBox(height: 4),
        Text(
          '${l10n.avalancheRegionLabel}: ${w.regionName}',
          style: theme.textTheme.bodyMedium?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 16),
        Text(
          _levelLabel(context, w.dangerLevel),
          style: theme.textTheme.titleMedium?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
        if (w.mainText != null) ...[
          const SizedBox(height: 16),
          Text(w.mainText!, style: theme.textTheme.bodyMedium),
        ],
        if (w.problems.isNotEmpty) ...[
          const SizedBox(height: 16),
          Text(
            l10n.avalancheProblemsTitle,
            style: theme.textTheme.titleSmall,
          ),
          const SizedBox(height: 8),
          for (final p in w.problems)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Text(
                [
                  if (p.typeName != null) p.typeName,
                  if (p.size != null) p.size,
                  if (p.sensitivity != null) p.sensitivity,
                ].whereType<String>().join(' · '),
                style: theme.textTheme.bodySmall,
              ),
            ),
        ],
        const SizedBox(height: 24),
        Text(
          l10n.avalancheAttribution,
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ],
    );
  }

  static String _levelLabel(BuildContext c, AvalancheDangerLevel l) {
    switch (l) {
      case AvalancheDangerLevel.low:
        return c.l10n.avalancheDangerLevel1;
      case AvalancheDangerLevel.moderate:
        return c.l10n.avalancheDangerLevel2;
      case AvalancheDangerLevel.considerable:
        return c.l10n.avalancheDangerLevel3;
      case AvalancheDangerLevel.high:
        return c.l10n.avalancheDangerLevel4;
      case AvalancheDangerLevel.extreme:
        return c.l10n.avalancheDangerLevel5;
    }
  }
}
