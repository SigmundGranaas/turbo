import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/markers/api.dart' show Marker;

import '../api.dart';

/// Weather block rendered inside [MarkerInfoSheet].
///
/// Always shows the same shape: a now-cast, an hourly strip for the next 24
/// hours, and a daily strip for the next 9 days. Marine conditions appear
/// below atmospheric whenever MET has data for the coordinate — there is no
/// toggle: an inland marker simply won't render the marine block.
class WeatherSection extends ConsumerWidget {
  final Marker marker;
  const WeatherSection({super.key, required this.marker});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;

    final forecast = ref.watch(weatherForecastProvider(marker.position));

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Icon(Icons.cloud_outlined,
                size: 18, color: colorScheme.onSurfaceVariant),
            const SizedBox(width: 8),
            Text(l10n.weatherForecast, style: textTheme.titleSmall),
          ],
        ),
        const SizedBox(height: 8),
        forecast.when(
          loading: () => const _LoadingBody(),
          error: (_, _) => _ErrorBody(
            onRetry: () => ref
                .read(weatherForecastProvider(marker.position).notifier)
                .refresh(),
          ),
          data: (f) => _DataBody(forecast: f),
        ),
        const SizedBox(height: 8),
        _AttributionFooter(),
      ],
    );
  }
}

class _LoadingBody extends StatelessWidget {
  const _LoadingBody();

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.symmetric(vertical: 12),
      child: LinearProgressIndicator(),
    );
  }
}

class _ErrorBody extends StatelessWidget {
  final VoidCallback onRetry;
  const _ErrorBody({required this.onRetry});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 12),
      child: Row(
        children: [
          Icon(Icons.cloud_off_outlined, color: colorScheme.error),
          const SizedBox(width: 8),
          Expanded(child: Text(l10n.weatherLoadError)),
          TextButton(onPressed: onRetry, child: Text(l10n.weatherRetry)),
        ],
      ),
    );
  }
}

class _DataBody extends StatelessWidget {
  final WeatherForecast forecast;
  const _DataBody({required this.forecast});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (forecast.currentAtmospheric != null)
          _NowCast(point: forecast.currentAtmospheric!),
        const SizedBox(height: 12),
        _HourlyStrip(points: _next24h(forecast.atmospheric)),
        const SizedBox(height: 12),
        _DailyStrip(summaries: _firstN(forecast.dailySummaries(), 9)),
        if (forecast.hasMarineData) ...[
          const SizedBox(height: 12),
          _MarineBlock(point: forecast.currentMarine!),
        ],
      ],
    );
  }

  static List<AtmosphericPoint> _next24h(List<AtmosphericPoint> all) {
    if (all.length <= 24) return all;
    return all.sublist(0, 24);
  }

  static List<DailySummary> _firstN(List<DailySummary> all, int n) {
    if (all.length <= n) return all;
    return all.sublist(0, n);
  }
}

class _NowCast extends StatelessWidget {
  final AtmosphericPoint point;
  const _NowCast({required this.point});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final tempStr = '${point.airTemperatureC.toStringAsFixed(0)}°';
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Icon(_iconForSymbol(point.symbol1h),
            size: 40, color: colorScheme.primary),
        const SizedBox(width: 12),
        Text(tempStr,
            style: textTheme.displaySmall?.copyWith(
              fontWeight: FontWeight.w300,
              color: colorScheme.onSurface,
            )),
        const SizedBox(width: 16),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              _Chip(
                icon: Icons.air,
                label: l10n.weatherWindLabel,
                value: _formatWind(point),
              ),
              if ((point.precipitation1hMm ?? 0) > 0) ...[
                const SizedBox(height: 4),
                _Chip(
                  icon: point.isSnowing
                      ? Icons.ac_unit
                      : Icons.water_drop_outlined,
                  label: l10n.weatherPrecipitationLabel,
                  value:
                      '${point.precipitation1hMm!.toStringAsFixed(1)} mm/h',
                ),
              ],
            ],
          ),
        ),
      ],
    );
  }
}

class _HourlyStrip extends StatelessWidget {
  final List<AtmosphericPoint> points;
  const _HourlyStrip({required this.points});

  @override
  Widget build(BuildContext context) {
    if (points.isEmpty) return const SizedBox.shrink();
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return SizedBox(
      key: const Key('weather-hourly-strip'),
      height: 72,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        itemCount: points.length,
        separatorBuilder: (_, _) => const SizedBox(width: 12),
        itemBuilder: (context, i) {
          final p = points[i];
          final hour = DateFormat.j().format(p.timeUtc.toLocal());
          return Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Text(hour,
                  style: textTheme.bodySmall
                      ?.copyWith(color: colorScheme.onSurfaceVariant)),
              const SizedBox(height: 2),
              Icon(_iconForSymbol(p.symbol1h),
                  size: 20, color: colorScheme.onSurface),
              const SizedBox(height: 2),
              Text('${p.airTemperatureC.toStringAsFixed(0)}°',
                  style: textTheme.bodyMedium),
            ],
          );
        },
      ),
    );
  }
}

class _DailyStrip extends StatelessWidget {
  final List<DailySummary> summaries;
  const _DailyStrip({required this.summaries});

  @override
  Widget build(BuildContext context) {
    if (summaries.isEmpty) return const SizedBox.shrink();
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return SizedBox(
      key: const Key('weather-daily-strip'),
      height: 84,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        itemCount: summaries.length,
        separatorBuilder: (_, _) => const SizedBox(width: 12),
        itemBuilder: (context, i) {
          final s = summaries[i];
          final weekday = DateFormat.E().format(s.date);
          return Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Text(weekday,
                  style: textTheme.bodySmall
                      ?.copyWith(color: colorScheme.onSurfaceVariant)),
              const SizedBox(height: 2),
              Icon(_iconForSymbol(s.middaySymbol),
                  size: 22, color: colorScheme.onSurface),
              const SizedBox(height: 2),
              Text(
                '${s.maxTempC.toStringAsFixed(0)}° / ${s.minTempC.toStringAsFixed(0)}°',
                style: textTheme.bodySmall,
              ),
              if (s.precipitationTotalMm > 0)
                Text(
                  '${s.precipitationTotalMm.toStringAsFixed(1)} mm',
                  style: textTheme.bodySmall
                      ?.copyWith(color: colorScheme.primary),
                ),
            ],
          );
        },
      ),
    );
  }
}

class _MarineBlock extends StatelessWidget {
  final MarinePoint point;
  const _MarineBlock({required this.point});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final wave = point.waveHeightM;
    final water = point.seaWaterTemperatureC;
    return Container(
      key: const Key('weather-marine-block'),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: colorScheme.surfaceContainerHighest.withValues(alpha: 0.4),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        children: [
          Icon(Icons.waves, size: 18, color: colorScheme.primary),
          const SizedBox(width: 8),
          Text(l10n.weatherMarineSection, style: textTheme.titleSmall),
          const Spacer(),
          if (wave != null)
            _Chip(
              icon: Icons.water,
              label: l10n.weatherWaveHeightLabel,
              value: '${wave.toStringAsFixed(1)} m',
            ),
          if (water != null) ...[
            const SizedBox(width: 8),
            _Chip(
              icon: Icons.thermostat,
              label: l10n.weatherWaterTempLabel,
              value: '${water.toStringAsFixed(0)}°C',
            ),
          ],
        ],
      ),
    );
  }
}

class _Chip extends StatelessWidget {
  final IconData icon;
  final String label;
  final String value;
  const _Chip({required this.icon, required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 14, color: colorScheme.onSurfaceVariant),
        const SizedBox(width: 4),
        Text(label,
            style: textTheme.bodySmall
                ?.copyWith(color: colorScheme.onSurfaceVariant)),
        const SizedBox(width: 4),
        Text(value, style: textTheme.bodyMedium),
      ],
    );
  }
}

class _AttributionFooter extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return InkWell(
      onTap: () => launchUrl(
        Uri.parse('https://www.met.no/en/free-meteorological-data'),
        mode: LaunchMode.externalApplication,
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Text(
          l10n.weatherAttribution,
          style: textTheme.bodySmall
              ?.copyWith(color: colorScheme.onSurfaceVariant),
        ),
      ),
    );
  }
}

/// 8-point compass label from a "from" bearing.
String _compassDir(double deg) {
  const dirs = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];
  final idx = ((deg % 360) / 45).round() % 8;
  return dirs[idx];
}

String _formatWind(AtmosphericPoint p) {
  final dir = p.windFromDeg == null ? '' : ' ${_compassDir(p.windFromDeg!)}';
  return '${p.windSpeedMs.toStringAsFixed(1)} m/s$dir';
}

/// Maps a MET symbol code to a Material icon — a lossy stopgap until we bundle
/// MET's official SVGs (CC BY 4.0). Categorizes by token search so all ~90
/// variants land on something sensible.
IconData _iconForSymbol(WeatherSymbol? s) {
  if (s == null || s.isFallback) return Icons.cloud_queue;
  final c = s.code;
  if (c.contains('thunder')) return Icons.thunderstorm;
  if (c.contains('snow')) return Icons.ac_unit;
  if (c.contains('sleet')) return Icons.grain;
  if (c.contains('rain')) return Icons.umbrella;
  if (c.contains('fog')) return Icons.foggy;
  if (c.contains('clearsky')) {
    return c.contains('night') ? Icons.nightlight_round : Icons.wb_sunny;
  }
  if (c.contains('fair') || c.contains('partlycloudy')) return Icons.wb_cloudy;
  return Icons.cloud;
}
