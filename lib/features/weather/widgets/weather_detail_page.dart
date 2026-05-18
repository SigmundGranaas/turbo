import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/markers/api.dart' show Marker;

import '../api.dart';
import 'weather_widgets_internal.dart';

/// Full-page forecast pushed from [WeatherSummaryRow]. Shows the now-cast
/// card, the 24-hour hourly strip, a 9-day daily list, and (when present) the
/// marine block. Both this page and the summary row read the same
/// `weatherForecastProvider`, so opening this route is instant when the row
/// already has data cached.
class WeatherDetailPage extends ConsumerWidget {
  final Marker marker;
  const WeatherDetailPage({super.key, required this.marker});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final forecast = ref.watch(weatherForecastProvider(marker.position));

    return Scaffold(
      appBar: AppBar(
        title: Text(marker.title),
        bottom: PreferredSize(
          preferredSize: const Size.fromHeight(20),
          child: Padding(
            padding: const EdgeInsets.only(bottom: 8, left: 16),
            child: Align(
              alignment: Alignment.centerLeft,
              child: Text(
                l10n.weatherForecast,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
              ),
            ),
          ),
        ),
      ),
      body: forecast.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (_, _) => _ErrorBody(
          onRetry: () => ref
              .read(weatherForecastProvider(marker.position).notifier)
              .refresh(),
        ),
        data: (f) => _Body(forecast: f),
      ),
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
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.cloud_off_outlined, size: 48, color: colorScheme.error),
          const SizedBox(height: 12),
          Text(l10n.weatherLoadError),
          const SizedBox(height: 12),
          FilledButton(onPressed: onRetry, child: Text(l10n.weatherRetry)),
        ],
      ),
    );
  }
}

class _Body extends StatelessWidget {
  final WeatherForecast forecast;
  const _Body({required this.forecast});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
      children: [
        if (forecast.currentAtmospheric != null) ...[
          _NowCastCard(point: forecast.currentAtmospheric!),
          const SizedBox(height: 24),
        ],
        _SectionHeader(label: l10n.weatherNext24h),
        const SizedBox(height: 8),
        _HourlyStrip(points: _next24h(forecast.atmospheric)),
        const SizedBox(height: 24),
        _SectionHeader(label: l10n.weatherNext9days),
        const SizedBox(height: 8),
        _DailyList(summaries: _firstN(forecast.dailySummaries(), 9)),
        if (forecast.hasMarineData) ...[
          const SizedBox(height: 24),
          _SectionHeader(label: l10n.weatherMarineSection),
          const SizedBox(height: 8),
          _MarineBlock(point: forecast.currentMarine!),
        ],
        const SizedBox(height: 24),
        const _AttributionFooter(),
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

class _SectionHeader extends StatelessWidget {
  final String label;
  const _SectionHeader({required this.label});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Text(
      label,
      style: textTheme.labelLarge?.copyWith(color: colorScheme.onSurfaceVariant),
    );
  }
}

class _NowCastCard extends StatelessWidget {
  final AtmosphericPoint point;
  const _NowCastCard({required this.point});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Container(
      key: const Key('weather-detail-nowcast'),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: colorScheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(16),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          WeatherSymbolIcon(symbol: point.symbol1h, size: 72),
          const SizedBox(width: 16),
          Text(
            '${point.airTemperatureC.toStringAsFixed(0)}°',
            style: textTheme.displayMedium?.copyWith(
              fontWeight: FontWeight.w300,
              color: colorScheme.onSurface,
            ),
          ),
          const SizedBox(width: 20),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                WeatherChip(
                  icon: Icons.air,
                  label: l10n.weatherWindLabel,
                  value: formatWind(point),
                ),
                if ((point.precipitation1hMm ?? 0) > 0) ...[
                  const SizedBox(height: 6),
                  WeatherChip(
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
      ),
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
      key: const Key('weather-detail-hourly'),
      height: 92,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        itemCount: points.length,
        separatorBuilder: (_, _) => const SizedBox(width: 16),
        itemBuilder: (context, i) {
          final p = points[i];
          final hour = DateFormat.j().format(p.timeUtc.toLocal());
          return Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Text(hour,
                  style: textTheme.bodySmall
                      ?.copyWith(color: colorScheme.onSurfaceVariant)),
              const SizedBox(height: 4),
              WeatherSymbolIcon(symbol: p.symbol1h, size: 36),
              const SizedBox(height: 4),
              Text('${p.airTemperatureC.toStringAsFixed(0)}°',
                  style: textTheme.bodyMedium),
              if ((p.precipitation1hMm ?? 0) > 0)
                Text(
                  p.precipitation1hMm!.toStringAsFixed(1),
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

class _DailyList extends StatelessWidget {
  final List<DailySummary> summaries;
  const _DailyList({required this.summaries});

  @override
  Widget build(BuildContext context) {
    if (summaries.isEmpty) return const SizedBox.shrink();
    return Column(
      key: const Key('weather-detail-daily'),
      children: [
        for (final s in summaries) _DailyRow(summary: s),
      ],
    );
  }
}

class _DailyRow extends StatelessWidget {
  final DailySummary summary;
  const _DailyRow({required this.summary});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final weekday = DateFormat('EEE, MMM d').format(summary.date);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Row(
        children: [
          SizedBox(
            width: 90,
            child: Text(weekday,
                style: textTheme.bodyMedium
                    ?.copyWith(color: colorScheme.onSurface)),
          ),
          WeatherSymbolIcon(symbol: summary.middaySymbol, size: 32),
          const SizedBox(width: 12),
          SizedBox(
            width: 64,
            child: Text(
              summary.precipitationTotalMm > 0
                  ? '${summary.precipitationTotalMm.toStringAsFixed(1)} mm'
                  : '',
              style: textTheme.bodySmall
                  ?.copyWith(color: colorScheme.primary),
            ),
          ),
          const Spacer(),
          Text(
            '${summary.maxTempC.toStringAsFixed(0)}° / ${summary.minTempC.toStringAsFixed(0)}°',
            style: textTheme.bodyMedium,
          ),
        ],
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
    final colorScheme = Theme.of(context).colorScheme;
    final wave = point.waveHeightM;
    final water = point.seaWaterTemperatureC;
    return Container(
      key: const Key('weather-detail-marine'),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: colorScheme.surfaceContainerHighest.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Wrap(
        spacing: 16,
        runSpacing: 8,
        children: [
          if (wave != null)
            WeatherChip(
              icon: Icons.water,
              label: l10n.weatherWaveHeightLabel,
              value: '${wave.toStringAsFixed(1)} m',
            ),
          if (water != null)
            WeatherChip(
              icon: Icons.thermostat,
              label: l10n.weatherWaterTempLabel,
              value: '${water.toStringAsFixed(0)}°C',
            ),
          if (point.waveFromDeg != null)
            WeatherChip(
              icon: Icons.navigation,
              label: 'Dir',
              value: compassDir(point.waveFromDeg!),
            ),
        ],
      ),
    );
  }
}

class _AttributionFooter extends StatelessWidget {
  const _AttributionFooter();

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
