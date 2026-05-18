import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import '../data/avalanche_forecast_notifier.dart';
import '../models/avalanche_warning.dart';
import 'show_avalanche_warning_sheet.dart';

/// Compact danger-level chip rendered in the weather sheet header when
/// Varsom has a forecast for the requested coordinate.
class AvalancheWarningBadge extends ConsumerWidget {
  final LatLng position;
  const AvalancheWarningBadge({super.key, required this.position});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(avalancheForecastProvider(position));
    return async.when(
      loading: () => const SizedBox.shrink(),
      error: (_, _) => const SizedBox.shrink(),
      data: (w) {
        if (w == null) return const SizedBox.shrink();
        return _Badge(warning: w, position: position);
      },
    );
  }
}

class _Badge extends StatelessWidget {
  final AvalancheWarning warning;
  final LatLng position;
  const _Badge({required this.warning, required this.position});

  @override
  Widget build(BuildContext context) {
    final colors = _colorsFor(context, warning.dangerLevel);
    final label = _levelLabel(context, warning.dangerLevel);
    return Material(
      color: colors.bg,
      borderRadius: BorderRadius.circular(12),
      child: InkWell(
        key: const Key('avalanche-warning-badge'),
        borderRadius: BorderRadius.circular(12),
        onTap: () => showAvalancheWarningSheet(context, position),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.terrain, color: colors.fg, size: 18),
              const SizedBox(width: 6),
              Text(
                context.l10n.avalancheForecast,
                style: TextStyle(color: colors.fg),
              ),
              const SizedBox(width: 8),
              Text(
                label,
                style: TextStyle(
                  color: colors.fg,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ],
          ),
        ),
      ),
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

  static _DangerColors _colorsFor(
      BuildContext context, AvalancheDangerLevel level) {
    switch (level) {
      case AvalancheDangerLevel.low:
        return const _DangerColors(
          bg: Color(0xFFCCE6CC),
          fg: Color(0xFF005000),
        );
      case AvalancheDangerLevel.moderate:
        return const _DangerColors(
          bg: Color(0xFFFFF6D9),
          fg: Color(0xFF6B5300),
        );
      case AvalancheDangerLevel.considerable:
        return const _DangerColors(
          bg: Color(0xFFFFE0CC),
          fg: Color(0xFF7A3B00),
        );
      case AvalancheDangerLevel.high:
        return const _DangerColors(
          bg: Color(0xFFFFCCCC),
          fg: Color(0xFF800000),
        );
      case AvalancheDangerLevel.extreme:
        return const _DangerColors(
          bg: Color(0xFF202020),
          fg: Color(0xFFFFFFFF),
        );
    }
  }
}

class _DangerColors {
  final Color bg;
  final Color fg;
  const _DangerColors({required this.bg, required this.fg});
}
