import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/markers/api.dart' show Marker;

import '../api.dart';
import 'weather_widgets_internal.dart';

/// Single-row weather summary embedded in `MarkerInfoSheet`. The row is a
/// surface — tapping it pushes [WeatherDetailPage] with the full forecast.
///
/// States the row can be in:
/// - Loading: shows a quiet placeholder line.
/// - Error: shows a compact error + tap-to-retry.
/// - Data with current point: symbol + temperature + summary + chevron.
/// - Data with no current point: hides the row (no atmospheric data).
class WeatherSummaryRow extends ConsumerWidget {
  final Marker marker;
  const WeatherSummaryRow({super.key, required this.marker});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final forecast = ref.watch(weatherForecastProvider(marker.position));
    return forecast.when(
      loading: () => const _LoadingRow(),
      error: (_, _) => _ErrorRow(
        onRetry: () => ref
            .read(weatherForecastProvider(marker.position).notifier)
            .refresh(),
      ),
      data: (f) {
        final now = f.currentAtmospheric;
        if (now == null) return const SizedBox.shrink();
        return _DataRow(marker: marker, point: now);
      },
    );
  }
}

class _DataRow extends StatelessWidget {
  final Marker marker;
  final AtmosphericPoint point;
  const _DataRow({required this.marker, required this.point});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    return Material(
      color: colorScheme.surfaceContainerHigh,
      borderRadius: BorderRadius.circular(12),
      child: InkWell(
        key: const Key('weather-summary-row'),
        borderRadius: BorderRadius.circular(12),
        onTap: () => Navigator.of(context).push(
          MaterialPageRoute(
            builder: (_) => WeatherDetailPage(marker: marker),
          ),
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          child: Row(
            children: [
              WeatherSymbolIcon(symbol: point.symbol1h, size: 36),
              const SizedBox(width: 12),
              Text(
                '${point.airTemperatureC.toStringAsFixed(0)}°',
                style: textTheme.titleLarge?.copyWith(
                  fontWeight: FontWeight.w400,
                  color: colorScheme.onSurface,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  nowcastSummary(point),
                  overflow: TextOverflow.ellipsis,
                  style: textTheme.bodyMedium
                      ?.copyWith(color: colorScheme.onSurfaceVariant),
                ),
              ),
              Icon(
                Icons.chevron_right,
                color: colorScheme.onSurfaceVariant,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _LoadingRow extends StatelessWidget {
  const _LoadingRow();

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    return Container(
      key: const Key('weather-summary-loading'),
      height: 56,
      decoration: BoxDecoration(
        color: colorScheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(12),
      ),
      padding: const EdgeInsets.symmetric(horizontal: 12),
      child: Row(
        children: [
          SizedBox(
            width: 18,
            height: 18,
            child: CircularProgressIndicator(
              strokeWidth: 2,
              color: colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(width: 16),
          Text(
            context.l10n.weatherForecast,
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: colorScheme.onSurfaceVariant,
                ),
          ),
        ],
      ),
    );
  }
}

class _ErrorRow extends StatelessWidget {
  final VoidCallback onRetry;
  const _ErrorRow({required this.onRetry});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final colorScheme = Theme.of(context).colorScheme;
    return Material(
      color: colorScheme.surfaceContainerHigh,
      borderRadius: BorderRadius.circular(12),
      child: InkWell(
        key: const Key('weather-summary-error'),
        borderRadius: BorderRadius.circular(12),
        onTap: onRetry,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 14),
          child: Row(
            children: [
              Icon(Icons.cloud_off_outlined, color: colorScheme.error),
              const SizedBox(width: 12),
              Expanded(child: Text(l10n.weatherLoadError)),
              Text(
                l10n.weatherRetry,
                style: TextStyle(color: colorScheme.primary),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
