import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/features/settings/api.dart';

import '../data/saved_path_repository.dart';
import '../data/trip_stats.dart';

/// Aggregate stats across every saved path. Lightweight derived view —
/// nothing here mutates state; we recompute from the repository each build.
class TripStatsPage extends ConsumerWidget {
  const TripStatsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final async = ref.watch(savedPathRepositoryProvider);
    final unit =
        ref.watch(settingsProvider).value?.distanceUnit ?? DistanceUnit.metric;

    return Scaffold(
      appBar: AppBar(title: const Text('Trip statistics')),
      body: async.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (_, _) => Center(child: Text(context.l10n.genericLoadError)),
        data: (paths) {
          final stats = TripStats.from(paths);
          if (stats.totalPaths == 0) {
            return const Center(
              child: Padding(
                padding: EdgeInsets.all(AppSpacing.xl),
                child: Text(
                  'No saved paths yet. Record a hike or import a GPX to see '
                  'stats here.',
                  textAlign: TextAlign.center,
                ),
              ),
            );
          }
          return Padding(
            padding: const EdgeInsets.all(AppSpacing.l),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                _StatCard(
                  icon: Icons.route,
                  label: 'Total distance',
                  value: formatDistance(stats.totalDistanceMeters, unit),
                ),
                const SizedBox(height: AppSpacing.m),
                _StatCard(
                  icon: Icons.trending_up,
                  label: 'Total ascent',
                  value: _formatElevation(stats.totalAscentMeters, unit),
                  subtitle: stats.recordedPaths == 0
                      ? 'Available once you record a hike'
                      : null,
                ),
                const SizedBox(height: AppSpacing.m),
                _StatCard(
                  icon: Icons.straighten,
                  label: 'Longest path',
                  value: formatDistance(stats.longestPathMeters, unit),
                ),
                const SizedBox(height: AppSpacing.m),
                _StatCard(
                  icon: Icons.timer_outlined,
                  label: 'Total moving time',
                  value: _formatDuration(
                      Duration(seconds: stats.totalMovingTimeSeconds)),
                  subtitle: stats.recordedPaths == 0
                      ? 'Available once you record a hike'
                      : null,
                ),
                const SizedBox(height: AppSpacing.m),
                _StatCard(
                  icon: Icons.event_available,
                  label: 'Days active',
                  value: '${stats.distinctRecordingDays}',
                ),
                const SizedBox(height: AppSpacing.m),
                _StatCard(
                  icon: Icons.bookmarks_outlined,
                  label: 'Paths saved',
                  value: '${stats.totalPaths}',
                  subtitle: stats.recordedPaths > 0
                      ? '${stats.recordedPaths} recorded · '
                          '${stats.totalPaths - stats.recordedPaths} imported or measured'
                      : null,
                ),
              ],
            ),
          );
        },
      ),
    );
  }

  String _formatElevation(double meters, DistanceUnit unit) {
    if (unit == DistanceUnit.imperial) {
      return '${(meters / 0.3048).round()} ft';
    }
    return '${meters.round()} m';
  }

  String _formatDuration(Duration d) {
    final h = d.inHours;
    final m = d.inMinutes.remainder(60);
    if (h == 0 && m == 0) return '—';
    if (h == 0) return '${m}m';
    return '${h}h ${m}m';
  }
}

class _StatCard extends StatelessWidget {
  final IconData icon;
  final String label;
  final String value;
  final String? subtitle;

  const _StatCard({
    required this.icon,
    required this.label,
    required this.value,
    this.subtitle,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Material(
      color: theme.colorScheme.surfaceContainer,
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.all(AppSpacing.l),
        child: Row(
          children: [
            Container(
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: theme.colorScheme.primaryContainer,
                borderRadius: BorderRadius.circular(10),
              ),
              child: Icon(icon, color: theme.colorScheme.onPrimaryContainer),
            ),
            const SizedBox(width: AppSpacing.l),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(label,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      )),
                  Text(value, style: theme.textTheme.headlineSmall),
                  if (subtitle != null)
                    Padding(
                      padding: const EdgeInsets.only(top: 2),
                      child: Text(
                        subtitle!,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
