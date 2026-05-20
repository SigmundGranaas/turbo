import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/backcountry_ski_repository.dart';
import '../models/backcountry_ski_conditions_report.dart';

/// Typed conditions panel for a backcountry ski route. Renders weather
/// chips like the fishing panel, plus a dedicated avalanche row that
/// shows the level + summary when Varsom data is available and a
/// "check Varsom before going" hint when it isn't yet. The
/// kind-specific advisor (server-side) already nudges the rationale
/// the same way; this panel reinforces it visually.
class BackcountrySkiConditionsPanel extends ConsumerWidget {
  final String activityId;
  const BackcountrySkiConditionsPanel({super.key, required this.activityId});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(backcountrySkiConditionsProvider(activityId));
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 8),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(children: [
              const Icon(Icons.snowing, size: 20),
              const SizedBox(width: 8),
              Text('Conditions', style: Theme.of(context).textTheme.titleMedium),
              const Spacer(),
              IconButton(
                icon: const Icon(Icons.refresh, size: 18),
                tooltip: 'Refresh',
                onPressed: () =>
                    ref.invalidate(backcountrySkiConditionsProvider(activityId)),
              ),
            ]),
            const SizedBox(height: 8),
            async.when(
              loading: () => const Padding(
                padding: EdgeInsets.symmetric(vertical: 16),
                child: Center(child: SizedBox(
                  width: 18, height: 18,
                  child: CircularProgressIndicator(strokeWidth: 2))),
              ),
              error: (e, _) => Text('Conditions unavailable: $e',
                style: TextStyle(color: Theme.of(context).colorScheme.error)),
              data: (report) => _Body(report: report),
            ),
          ],
        ),
      ),
    );
  }
}

class _Body extends StatelessWidget {
  final BackcountrySkiConditionsReport report;
  const _Body({required this.report});

  @override
  Widget build(BuildContext context) {
    final w = report.weather;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (report.score != null) _ScoreBadge(score: report.score!),
        if (report.score != null) const SizedBox(height: 8),
        _AvalancheRow(level: report.avalancheLevel, summary: report.avalancheSummary),
        const SizedBox(height: 8),
        Wrap(spacing: 8, runSpacing: 6, children: [
          _Chip(icon: Icons.thermostat_outlined,
            label: '${w.airTemperatureCelsius.toStringAsFixed(1)}°C'),
          _Chip(icon: Icons.air_outlined,
            label: '${w.windSpeedMs.toStringAsFixed(1)} m/s'
                '${w.windGustMs != null ? " · gust ${w.windGustMs!.toStringAsFixed(0)}" : ""}'),
          _Chip(icon: Icons.cloud_outlined,
            label: '${w.cloudCoveragePct.toStringAsFixed(0)}% cloud'),
          if (w.precipitationNext6hMm != null && w.precipitationNext6hMm! > 0)
            _Chip(icon: Icons.ac_unit_outlined,
              label: '${w.precipitationNext6hMm!.toStringAsFixed(0)} mm/6h'),
          _Chip(icon: Icons.compress_outlined,
            label: '${w.airPressureHpa.toStringAsFixed(0)} hPa'),
        ]),
        const SizedBox(height: 8),
        Text(report.rationale, style: Theme.of(context).textTheme.bodySmall),
      ],
    );
  }
}

class _AvalancheRow extends StatelessWidget {
  final int? level;
  final String? summary;
  const _AvalancheRow({required this.level, required this.summary});

  @override
  Widget build(BuildContext context) {
    if (level == null) {
      return Container(
        padding: const EdgeInsets.all(8),
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.surfaceContainerHighest,
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(children: [
          Icon(Icons.info_outline, size: 18, color: Theme.of(context).colorScheme.onSurfaceVariant),
          const SizedBox(width: 8),
          const Expanded(child: Text(
            'Avalanche data not yet available — check Varsom before going.',
            style: TextStyle(fontSize: 12))),
        ]),
      );
    }
    final color = _levelColor(level!);
    return Container(
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        border: Border.all(color: color.withValues(alpha: 0.4)),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Row(children: [
          Icon(Icons.warning_amber_outlined, color: color, size: 18),
          const SizedBox(width: 8),
          Text('Avalanche level $level — ${_levelLabel(level!)}',
            style: TextStyle(color: color, fontWeight: FontWeight.w600)),
        ]),
        if (summary != null && summary!.isNotEmpty) ...[
          const SizedBox(height: 4),
          Text(summary!, style: const TextStyle(fontSize: 12)),
        ],
      ]),
    );
  }

  static Color _levelColor(int level) => switch (level) {
        1 => Colors.green,
        2 => Colors.lightGreen,
        3 => Colors.amber.shade700,
        4 => Colors.orange.shade800,
        5 => Colors.red.shade800,
        _ => Colors.grey,
      };

  static String _levelLabel(int level) => switch (level) {
        1 => 'low',
        2 => 'moderate',
        3 => 'considerable',
        4 => 'high',
        5 => 'extreme',
        _ => 'unknown',
      };
}

class _ScoreBadge extends StatelessWidget {
  final int score;
  const _ScoreBadge({required this.score});

  @override
  Widget build(BuildContext context) {
    final color = score >= 75 ? Colors.green : score >= 50 ? Colors.amber.shade700 : Colors.red;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: color.withValues(alpha: 0.4))),
      child: Row(mainAxisSize: MainAxisSize.min, children: [
        Icon(Icons.outlined_flag, color: color, size: 16),
        const SizedBox(width: 6),
        Text('Score $score / 100', style: TextStyle(color: color, fontWeight: FontWeight.w600)),
      ]),
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
          borderRadius: BorderRadius.circular(12)),
        child: Row(mainAxisSize: MainAxisSize.min, children: [
          Icon(icon, size: 14),
          const SizedBox(width: 4),
          Text(label, style: Theme.of(context).textTheme.bodySmall),
        ]),
      );
}
