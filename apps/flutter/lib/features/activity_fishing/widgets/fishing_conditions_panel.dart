import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:turbo/features/activities/api.dart' show ActivityConditionsMap;

import '../data/fishing_repository.dart';
import '../models/fishing_conditions_report.dart';

/// Typed conditions panel for a fishing spot. Renders the server's
/// scored report — score badge, weather chips, free-form rationale.
/// The kind-specific advisor (server-side) decides what counts as a
/// good condition; this panel just paints what it gets.
class FishingConditionsPanel extends ConsumerWidget {
  final String activityId;
  const FishingConditionsPanel({super.key, required this.activityId});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(fishingConditionsProvider(activityId));
    final activityAsync = ref.watch(fishingActivityProvider(activityId));
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 8),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                const Icon(Icons.cloud_outlined, size: 20),
                const SizedBox(width: 8),
                Text('Conditions', style: Theme.of(context).textTheme.titleMedium),
                const Spacer(),
                IconButton(
                  icon: const Icon(Icons.refresh, size: 18),
                  tooltip: 'Refresh',
                  onPressed: () => ref.invalidate(fishingConditionsProvider(activityId)),
                ),
              ],
            ),
            const SizedBox(height: 8),
            async.when(
              loading: () => const Padding(
                padding: EdgeInsets.symmetric(vertical: 16),
                child: Center(child: SizedBox(
                  width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2))),
              ),
              error: (e, _) => Text(
                'Conditions unavailable: $e',
                style: TextStyle(color: Theme.of(context).colorScheme.error)),
              data: (report) {
                final activity = activityAsync.value;
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    if (activity != null) ...[
                      ActivityConditionsMap(
                        points: [activity.position],
                        tintColor: const Color(0xFF1E6FB8),
                        airTemperatureCelsius: report.weather.airTemperatureCelsius,
                        cloudCoveragePct: report.weather.cloudCoveragePct,
                        windFromDegrees: report.weather.windFromDegrees,
                        windSpeedMs: report.weather.windSpeedMs,
                        precipitationNext1hMm: report.weather.precipitationNext1hMm,
                        symbolCode: report.weather.symbolCode,
                      ),
                      const SizedBox(height: 12),
                    ],
                    _Body(report: report),
                  ],
                );
              },
            ),
          ],
        ),
      ),
    );
  }
}

class _Body extends StatelessWidget {
  final FishingConditionsReport report;
  const _Body({required this.report});

  @override
  Widget build(BuildContext context) {
    final w = report.weather;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (report.score != null) _ScoreBadge(score: report.score!),
        if (report.score != null) const SizedBox(height: 8),
        Wrap(
          spacing: 8, runSpacing: 6,
          children: [
            _Chip(icon: Icons.thermostat_outlined,
              label: '${w.airTemperatureCelsius.toStringAsFixed(1)}°C'),
            _Chip(icon: Icons.compress_outlined,
              label: '${w.airPressureHpa.toStringAsFixed(0)} hPa'),
            _Chip(icon: Icons.air_outlined,
              label: '${w.windSpeedMs.toStringAsFixed(1)} m/s'
                  '${w.windGustMs != null ? " · gust ${w.windGustMs!.toStringAsFixed(0)}" : ""}'),
            _Chip(icon: Icons.cloud_outlined,
              label: '${w.cloudCoveragePct.toStringAsFixed(0)}% cloud'),
            if (w.precipitationNext1hMm != null && w.precipitationNext1hMm! > 0)
              _Chip(icon: Icons.water_drop_outlined,
                label: '${w.precipitationNext1hMm!.toStringAsFixed(1)} mm/h'),
            _Chip(icon: Icons.water_outlined,
              label: '${w.relativeHumidityPct.toStringAsFixed(0)}% RH'),
          ],
        ),
        const SizedBox(height: 8),
        Text(report.rationale, style: Theme.of(context).textTheme.bodySmall),
        const SizedBox(height: 4),
        Text(
          'Valid ${_fmt(report.validAt.toLocal())} · '
          'fetched ${_fmt(report.fetchedAt.toLocal())}',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
            color: Theme.of(context).colorScheme.onSurfaceVariant)),
      ],
    );
  }

  static String _fmt(DateTime dt) {
    String two(int n) => n.toString().padLeft(2, '0');
    return '${two(dt.hour)}:${two(dt.minute)}';
  }
}

class _ScoreBadge extends StatelessWidget {
  final int score;
  const _ScoreBadge({required this.score});

  @override
  Widget build(BuildContext context) {
    final color = score >= 75
        ? Colors.green
        : score >= 50
            ? Colors.amber.shade700
            : Colors.red;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.outlined_flag, color: color, size: 16),
          const SizedBox(width: 6),
          Text('Score $score / 100', style: TextStyle(color: color, fontWeight: FontWeight.w600)),
        ],
      ),
    );
  }
}

class _Chip extends StatelessWidget {
  final IconData icon;
  final String label;
  const _Chip({required this.icon, required this.label});

  @override
  Widget build(BuildContext context) => Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.surfaceContainerHighest,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 14),
            const SizedBox(width: 4),
            Text(label, style: Theme.of(context).textTheme.bodySmall),
          ],
        ),
      );
}
