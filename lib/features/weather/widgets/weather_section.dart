import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/markers/api.dart' show Marker;

import '../api.dart';

/// Compact, customizable weather block rendered inside [MarkerInfoSheet].
///
/// Watches the marker's metric preferences and the merged forecast for its
/// position. Renders only the rows the user opted into. Marine rows are
/// silently hidden when MET has no data for the coordinate.
class WeatherSection extends ConsumerWidget {
  final Marker marker;
  const WeatherSection({super.key, required this.marker});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final prefs = ref.watch(markerWeatherPrefsProvider(marker.uuid));
    final sources = WeatherMetric.sourcesFor(prefs.metrics);

    if (sources.isEmpty) {
      // Nothing opted in. Show a hairline header + customize affordance.
      return _SectionFrame(
        markerUuid: marker.uuid,
        l10n: l10n,
        child: const SizedBox.shrink(),
      );
    }

    final request =
        WeatherRequest(position: marker.position, sources: sources);
    final forecast = ref.watch(weatherForecastProvider(request));

    return _SectionFrame(
      markerUuid: marker.uuid,
      l10n: l10n,
      child: forecast.when(
        loading: () => const _LoadingBody(),
        error: (e, _) => _ErrorBody(
          onRetry: () =>
              ref.read(weatherForecastProvider(request).notifier).refresh(),
        ),
        data: (f) => _DataBody(prefs: prefs, forecast: f),
      ),
    );
  }
}

class _SectionFrame extends StatelessWidget {
  final String markerUuid;
  final AppLocalizations l10n;
  final Widget child;
  const _SectionFrame({
    required this.markerUuid,
    required this.l10n,
    required this.child,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Icon(Icons.cloud_outlined,
                size: 18, color: colorScheme.onSurfaceVariant),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                l10n.weatherForecast,
                style: textTheme.titleSmall,
              ),
            ),
            IconButton(
              tooltip: l10n.weatherCustomize,
              iconSize: 20,
              visualDensity: VisualDensity.compact,
              icon: const Icon(Icons.tune),
              onPressed: () => _openCustomize(context, markerUuid),
            ),
          ],
        ),
        child,
        const SizedBox(height: 8),
        InkWell(
          onTap: () => launchUrl(
            Uri.parse('https://www.met.no/en/free-meteorological-data'),
            mode: LaunchMode.externalApplication,
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 4),
            child: Text(
              l10n.weatherAttribution,
              style: textTheme.bodySmall?.copyWith(
                color: colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        ),
      ],
    );
  }

  void _openCustomize(BuildContext context, String markerUuid) {
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (_) => WeatherMetricsSheet(markerUuid: markerUuid),
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
  final MarkerWeatherPrefs prefs;
  final WeatherForecast forecast;
  const _DataBody({required this.prefs, required this.forecast});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final rows = <Widget>[];
    final hasMarine = forecast.hasMarineData;

    for (final metric in WeatherMetric.values) {
      if (!prefs.metrics.contains(metric)) continue;
      if (metric.source == WeatherMetricSource.marine && !hasMarine) continue;
      rows.add(_MetricRow(
        label: _label(l10n, metric),
        value: _currentValue(forecast, metric, l10n),
      ));
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: rows,
    );
  }
}

class _MetricRow extends StatelessWidget {
  final String label;
  final String value;
  const _MetricRow({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          Expanded(
            child: Text(
              label,
              style: textTheme.bodyMedium?.copyWith(
                color: colorScheme.onSurfaceVariant,
              ),
            ),
          ),
          Text(value, style: textTheme.bodyMedium),
        ],
      ),
    );
  }
}

String _label(AppLocalizations l10n, WeatherMetric metric) {
  switch (metric) {
    case WeatherMetric.temperature:
      return l10n.metricTemperature;
    case WeatherMetric.precipitation:
      return l10n.metricPrecipitation;
    case WeatherMetric.snow:
      return l10n.metricSnow;
    case WeatherMetric.wind:
      return l10n.metricWind;
    case WeatherMetric.humidity:
      return l10n.metricHumidity;
    case WeatherMetric.pressure:
      return l10n.metricPressure;
    case WeatherMetric.cloudCover:
      return l10n.metricCloudCover;
    case WeatherMetric.uvIndex:
      return l10n.metricUv;
    case WeatherMetric.waveHeight:
      return l10n.metricWaveHeight;
    case WeatherMetric.waveDirection:
      return l10n.metricWaveDirection;
    case WeatherMetric.waterTemperature:
      return l10n.metricWaterTemperature;
  }
}

String _currentValue(
    WeatherForecast f, WeatherMetric metric, AppLocalizations l10n) {
  final atm = f.currentAtmospheric;
  final marine = f.currentMarine;
  switch (metric) {
    case WeatherMetric.temperature:
      return atm == null ? '—' : '${atm.airTemperatureC.toStringAsFixed(1)}°C';
    case WeatherMetric.precipitation:
      final p = atm?.precipitation1hMm;
      return p == null ? '—' : '${p.toStringAsFixed(1)} mm/h';
    case WeatherMetric.snow:
      final p = atm?.precipitation1hMm;
      final snowing = atm?.isSnowing ?? false;
      if (p == null || !snowing) return '—';
      return '${p.toStringAsFixed(1)} mm/h';
    case WeatherMetric.wind:
      if (atm == null) return '—';
      final dir = atm.windFromDeg == null
          ? ''
          : ' ${_compassDir(atm.windFromDeg!)}';
      return '${atm.windSpeedMs.toStringAsFixed(1)} m/s$dir';
    case WeatherMetric.humidity:
      final h = atm?.humidity;
      return h == null ? '—' : '${h.toStringAsFixed(0)}%';
    case WeatherMetric.pressure:
      final p = atm?.pressureHpa;
      return p == null ? '—' : '${p.toStringAsFixed(0)} hPa';
    case WeatherMetric.cloudCover:
      final c = atm?.cloudCoverPercent;
      return c == null ? '—' : '${c.toStringAsFixed(0)}%';
    case WeatherMetric.uvIndex:
      final u = atm?.uvIndex;
      return u == null ? '—' : u.toStringAsFixed(1);
    case WeatherMetric.waveHeight:
      final h = marine?.waveHeightM;
      return h == null ? '—' : '${h.toStringAsFixed(1)} m';
    case WeatherMetric.waveDirection:
      final d = marine?.waveFromDeg;
      return d == null ? '—' : _compassDir(d);
    case WeatherMetric.waterTemperature:
      final t = marine?.seaWaterTemperatureC;
      return t == null ? '—' : '${t.toStringAsFixed(1)}°C';
  }
}

/// 8-point compass label from a "from" bearing (0° = wind from north).
String _compassDir(double deg) {
  const dirs = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];
  final idx = ((deg % 360) / 45).round() % 8;
  return dirs[idx];
}
