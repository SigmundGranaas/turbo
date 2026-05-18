import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/location/location_state.dart';
import 'package:turbo/core/widgets/app_pill.dart';
import 'package:turbo/features/settings/api.dart';

import '../api.dart';
import 'weather_widgets_internal.dart';

/// Compact wind/gust readout for the top of the map view. Reads the same GPS
/// stream via [lastPositionProvider] so it follows the user (rounded to ~1 km
/// to avoid refetching MET on every GPS tick), then renders:
///
///   ▲ 6.2 m/s (gust 9.4) │ ▁▂▃▃▂▂   (next 6 h)
///
/// Gated by `SettingsState.showWindStrip` — collapses to `SizedBox.shrink`
/// when off so it costs nothing in the layout.
class MarineWindStrip extends ConsumerWidget {
  const MarineWindStrip({super.key});

  /// Round a coordinate to ~1 km granularity so small GPS drift doesn't churn
  /// the weatherForecastProvider family key.
  static LatLng _quantize(LatLng raw) {
    double q(double v) => (v * 100).round() / 100;
    return LatLng(q(raw.latitude), q(raw.longitude));
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final enabled = ref.watch(settingsProvider
        .select((s) => s.value?.showWindStrip ?? false));
    if (!enabled) return const SizedBox.shrink();

    final snapshot = ref.watch(lastPositionProvider);
    final l10n = AppLocalizations.of(context);
    if (snapshot == null) {
      return _StripFrame(child: _PlaceholderText(text: l10n.windStripNoData));
    }

    final position = _quantize(snapshot.latLng);
    final forecast = ref.watch(weatherForecastProvider(position));

    return forecast.when(
      loading: () => _StripFrame(
        child: SizedBox(
          height: 18,
          width: 18,
          child: CircularProgressIndicator(
            strokeWidth: 2,
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
        ),
      ),
      error: (_, _) =>
          _StripFrame(child: _PlaceholderText(text: l10n.windStripNoData)),
      data: (f) {
        final current = f.currentAtmospheric;
        if (current == null) {
          return _StripFrame(
              child: _PlaceholderText(text: l10n.windStripNoData));
        }
        return _StripFrame(
          child: _WindContent(
            current: current,
            next6h: _take6Hourly(f.atmospheric),
            l10n: l10n,
          ),
        );
      },
    );
  }

  /// Pick the next 6 distinct-hour atmospheric points from the timeseries.
  /// The MET payload starts with the current hour; we keep the first point
  /// per local hour so a 6-bar strip covers a real 6-hour window.
  static List<AtmosphericPoint> _take6Hourly(List<AtmosphericPoint> all) {
    final out = <AtmosphericPoint>[];
    final seen = <int>{};
    for (final p in all) {
      final hour = p.timeUtc.millisecondsSinceEpoch ~/ (1000 * 60 * 60);
      if (seen.add(hour)) {
        out.add(p);
        if (out.length == 6) break;
      }
    }
    return out;
  }
}

class _StripFrame extends StatelessWidget {
  final Widget child;
  const _StripFrame({required this.child});

  @override
  Widget build(BuildContext context) {
    return AppPill(
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.l,
        vertical: AppSpacing.s,
      ),
      child: child,
    );
  }
}

class _PlaceholderText extends StatelessWidget {
  final String text;
  const _PlaceholderText({required this.text});

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(Icons.air, size: 16, color: colorScheme.onSurfaceVariant),
        const SizedBox(width: AppSpacing.s),
        Text(
          text,
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: colorScheme.onSurfaceVariant,
              ),
        ),
      ],
    );
  }
}

class _WindContent extends StatelessWidget {
  final AtmosphericPoint current;
  final List<AtmosphericPoint> next6h;
  final AppLocalizations l10n;

  const _WindContent({
    required this.current,
    required this.next6h,
    required this.l10n,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        WindArrow(fromDeg: current.windFromDeg, size: 18),
        const SizedBox(width: AppSpacing.s),
        Text(
          '${current.windSpeedMs.toStringAsFixed(1)} m/s',
          style: textTheme.titleSmall?.copyWith(
            fontFeatures: const [FontFeature.tabularFigures()],
            fontWeight: FontWeight.w600,
          ),
        ),
        if (current.windGustMs != null) ...[
          const SizedBox(width: AppSpacing.xs),
          Text(
            '(${l10n.windStripGustLabel} ${current.windGustMs!.toStringAsFixed(1)})',
            style: textTheme.bodySmall?.copyWith(
              color: _gustColor(current.windGustMs!, colorScheme),
              fontFeatures: const [FontFeature.tabularFigures()],
              fontWeight: FontWeight.w500,
            ),
          ),
        ],
        if (next6h.isNotEmpty) ...[
          const SizedBox(width: AppSpacing.m),
          _Sparkline(points: next6h, colorScheme: colorScheme),
        ],
      ],
    );
  }

  /// Tint the gust readout when the value crosses small-boat comfort
  /// thresholds: amber from 10 m/s (~20 kn), error from 15 m/s (~30 kn).
  Color _gustColor(double gustMs, ColorScheme scheme) {
    if (gustMs >= 15) return scheme.error;
    if (gustMs >= 10) return scheme.tertiary;
    return scheme.onSurfaceVariant;
  }
}

/// Six fixed-width bars whose heights are normalized against the maximum
/// observed wind speed in the same window — purely a relative trend
/// indicator, not a calibrated y-axis.
class _Sparkline extends StatelessWidget {
  final List<AtmosphericPoint> points;
  final ColorScheme colorScheme;

  const _Sparkline({required this.points, required this.colorScheme});

  @override
  Widget build(BuildContext context) {
    final maxSpeed = points
        .map((p) => p.windSpeedMs)
        .fold<double>(0.1, (a, b) => a > b ? a : b);
    return SizedBox(
      height: 22,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          for (final p in points) ...[
            _Bar(
              speedMs: p.windSpeedMs,
              normalized: (p.windSpeedMs / maxSpeed).clamp(0.05, 1.0),
              colorScheme: colorScheme,
            ),
            const SizedBox(width: 2),
          ],
        ],
      ),
    );
  }
}

class _Bar extends StatelessWidget {
  final double speedMs;
  final double normalized;
  final ColorScheme colorScheme;

  const _Bar({
    required this.speedMs,
    required this.normalized,
    required this.colorScheme,
  });

  @override
  Widget build(BuildContext context) {
    final color = speedMs >= 12
        ? colorScheme.error
        : speedMs >= 8
            ? colorScheme.tertiary
            : colorScheme.primary;
    return Container(
      width: 4,
      height: 22 * normalized,
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.85),
        borderRadius: BorderRadius.circular(2),
      ),
    );
  }
}
