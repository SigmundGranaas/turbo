import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/weather/api.dart' show weatherForecastProvider;
import '../data/avalanche_forecast_notifier.dart';
import '../models/avalanche_warning.dart';
import 'show_avalanche_warning_sheet.dart';

/// Full-width banner card rendered inside the weather sheet when Varsom has
/// a relevant forecast for the queried coordinate. Tap to open the detail
/// sheet.
///
/// Display is gated by [shouldShowAvalancheWarning]: the widget reads the
/// matching weather forecast so it can hide low-severity warnings at warm
/// locations.
class AvalancheWarningBadge extends ConsumerWidget {
  final LatLng position;
  const AvalancheWarningBadge({super.key, required this.position});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(avalancheForecastProvider(position));
    final weather = ref.watch(weatherForecastProvider(position));
    return async.when(
      loading: () => const SizedBox.shrink(),
      error: (_, _) => const SizedBox.shrink(),
      data: (w) {
        if (w == null) return const SizedBox.shrink();
        final temp = weather.asData?.value.currentAtmospheric?.airTemperatureC;
        if (!shouldShowAvalancheWarning(w, currentAirTempC: temp)) {
          return const SizedBox.shrink();
        }
        return _Card(warning: w, position: position);
      },
    );
  }
}

class _Card extends StatelessWidget {
  final AvalancheWarning warning;
  final LatLng position;
  const _Card({required this.warning, required this.position});

  @override
  Widget build(BuildContext context) {
    final colors = _colorsFor(context, warning.dangerLevel);
    final levelLabel = _levelLabel(context, warning.dangerLevel);
    final tt = Theme.of(context).textTheme;

    return Material(
      color: colors.bg,
      borderRadius: BorderRadius.circular(12),
      child: InkWell(
        key: const Key('avalanche-warning-badge'),
        borderRadius: BorderRadius.circular(12),
        onTap: () => showAvalancheWarningSheet(context, position),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          child: Row(
            children: [
              Icon(Icons.terrain, color: colors.fg, size: 22),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      context.l10n.avalancheForecast,
                      style: tt.titleSmall?.copyWith(
                        color: colors.fg,
                        fontWeight: FontWeight.w600,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    Text(
                      '$levelLabel · ${warning.regionName}',
                      style: tt.bodySmall?.copyWith(
                        color: colors.fg.withValues(alpha: 0.85),
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ],
                ),
              ),
              Icon(Icons.chevron_right, color: colors.fg.withValues(alpha: 0.7)),
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
          bg: Color(0xFFD6EBD6),
          fg: Color(0xFF1B5E20),
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
        return _DangerColors(
          bg: Theme.of(context).colorScheme.errorContainer,
          fg: Theme.of(context).colorScheme.onErrorContainer,
        );
      case AvalancheDangerLevel.extreme:
        return const _DangerColors(
          bg: Color(0xFF1A1A1A),
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
