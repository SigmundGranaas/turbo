import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/markers/api.dart' as marker_model;

import '../api.dart';
import 'weather_widgets_internal.dart';

/// Single-row weather summary used by the marker info sheet and the
/// long-press pin sheet. Tapping the row opens the full forecast.
///
/// Takes a coordinate + title directly so callers don't have to mint a
/// transient [marker_model.Marker] just to delegate the forecast call.
/// The widget constructs a marker internally when it has to hand off to
/// [showWeatherDetailSheet] (which is keyed on `Marker`).
class WeatherSummaryRow extends ConsumerWidget {
  final LatLng position;
  final String title;
  const WeatherSummaryRow({
    super.key,
    required this.position,
    required this.title,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final forecast = ref.watch(weatherForecastProvider(position));
    return forecast.when(
      loading: () => const _LoadingRow(),
      error: (_, _) => _ErrorRow(
        onRetry: () => ref
            .read(weatherForecastProvider(position).notifier)
            .refresh(),
      ),
      data: (f) {
        final now = f.currentAtmospheric;
        if (now == null) return const SizedBox.shrink();
        return _DataRow(position: position, title: title, point: now);
      },
    );
  }
}

class _DataRow extends StatelessWidget {
  final LatLng position;
  final String title;
  final AtmosphericPoint point;
  const _DataRow({
    required this.position,
    required this.title,
    required this.point,
  });

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
        onTap: () => showWeatherDetailSheet(
          context,
          marker_model.Marker(title: title, position: position),
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
              const Spacer(),
              WindReadout(
                point: point,
                arrowSize: 18,
                textStyle: textTheme.bodyMedium?.copyWith(
                  color: colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(width: 8),
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
